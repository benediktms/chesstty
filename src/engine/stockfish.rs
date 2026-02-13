use crate::engine::uci::{parse_uci_message, format_uci_move, UciMessage};
use crate::engine::{EngineCommand, EngineEvent, GoParams, Score};
use cozy_chess::Move;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

pub struct StockfishEngine {
    process: Child,
    stdin: mpsc::Sender<String>,
    command_tx: mpsc::Sender<EngineCommand>,
    event_rx: mpsc::Receiver<EngineEvent>,
}

impl StockfishEngine {
    /// Spawn a new Stockfish instance
    pub async fn spawn(skill_level: Option<u8>) -> Result<Self, String> {
        let path = find_stockfish_path().ok_or("Stockfish not found")?;

        let mut process = tokio::process::Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Stockfish: {}", e))?;

        let mut stdin = process.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = process.stdout.take().ok_or("Failed to get stdout")?;

        // Initialize UCI
        stdin
            .write_all(b"uci\n")
            .await
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
        stdin.flush().await.map_err(|e| format!("Failed to flush: {}", e))?;

        // Create channels for communication
        let (command_tx, mut command_rx) = mpsc::channel::<EngineCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<EngineEvent>(32);

        // Spawn output reader task
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if let Ok(msg) = parse_uci_message(line.trim()) {
                            let event = match msg {
                                UciMessage::UciOk => EngineEvent::Ready,
                                UciMessage::ReadyOk => EngineEvent::Ready,
                                UciMessage::BestMove { mv, .. } => EngineEvent::BestMove(mv),
                                UciMessage::Info(info) => EngineEvent::Info(info),
                                _ => continue,
                            };
                            let _ = event_tx_clone.send(event).await;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Wait for uciok
        let mut temp_rx = event_rx;
        while let Some(event) = temp_rx.recv().await {
            if matches!(event, EngineEvent::Ready) {
                break;
            }
        }
        let mut event_rx = temp_rx;

        // Set skill level if provided
        if let Some(level) = skill_level {
            stdin
                .write_all(format!("setoption name Skill Level value {}\n", level).as_bytes())
                .await
                .map_err(|e| format!("Failed to set skill level: {}", e))?;
            stdin.flush().await.map_err(|e| format!("Failed to flush: {}", e))?;
        }

        // Clone stdin for the command processor task
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);

        // Spawn stdin writer task
        tokio::spawn(async move {
            while let Some(cmd) = stdin_rx.recv().await {
                let _ = stdin.write_all(cmd.as_bytes()).await;
                let _ = stdin.flush().await;
            }
        });

        // Send isready
        let _ = stdin_tx.send("isready\n".to_string()).await;

        // Spawn command processor task
        let event_tx_for_commands = event_tx.clone();
        let stdin_tx_for_commands = stdin_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                let cmd_str = match cmd {
                    EngineCommand::SetPosition { fen, moves } => {
                        let mut position_cmd = format!("position fen {}", fen);
                        if !moves.is_empty() {
                            position_cmd.push_str(" moves");
                            for mv in moves {
                                position_cmd.push_str(&format!(" {}", format_uci_move(&mv)));
                            }
                        }
                        position_cmd.push('\n');
                        position_cmd
                    }
                    EngineCommand::Go(params) => {
                        let mut go_cmd = "go".to_string();
                        if let Some(movetime) = params.movetime {
                            go_cmd.push_str(&format!(" movetime {}", movetime));
                        } else if let Some(depth) = params.depth {
                            go_cmd.push_str(&format!(" depth {}", depth));
                        } else if params.infinite {
                            go_cmd.push_str(" infinite");
                        } else {
                            go_cmd.push_str(" movetime 1000"); // Default 1 second
                        }
                        go_cmd.push('\n');
                        go_cmd
                    }
                    EngineCommand::Stop => "stop\n".to_string(),
                    EngineCommand::Quit => {
                        let _ = stdin_tx_for_commands.send("quit\n".to_string()).await;
                        break;
                    }
                };
                let _ = stdin_tx_for_commands.send(cmd_str).await;
            }
        });

        Ok(Self {
            process,
            stdin: stdin_tx,
            command_tx,
            event_rx,
        })
    }

    /// Send a command to the engine
    pub async fn send_command(&self, cmd: EngineCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Try to receive an event from the engine (non-blocking)
    pub fn try_recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive an event from the engine (blocking)
    pub async fn recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx.recv().await
    }

    /// Shutdown the engine
    pub async fn shutdown(mut self) {
        let _ = self.send_command(EngineCommand::Quit).await;
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            self.process.wait()
        ).await;
        let _ = self.process.kill().await;
    }
}

/// Find Stockfish executable in common locations
fn find_stockfish_path() -> Option<PathBuf> {
    // Common paths to check
    let paths = vec![
        "/usr/local/bin/stockfish",
        "/usr/bin/stockfish",
        "/opt/homebrew/bin/stockfish",
        "/usr/games/stockfish",
        "stockfish", // In PATH
    ];

    for path_str in paths {
        let path = Path::new(path_str);
        if path.exists() || path_str == "stockfish" {
            // Try to verify it's actually stockfish
            if let Ok(_) = std::process::Command::new(path_str).arg("--help").output() {
                return Some(PathBuf::from(path_str));
            }
        }
    }

    None
}

/// Helper to make a move with the engine
pub async fn make_engine_move(
    engine: &StockfishEngine,
    fen: &str,
    moves: Vec<Move>,
    skill_level: u8,
) -> Result<Move, String> {
    // Set position
    engine
        .send_command(EngineCommand::SetPosition {
            fen: fen.to_string(),
            moves,
        })
        .await?;

    // Start thinking - adjust time based on skill level
    let movetime = match skill_level {
        0..=5 => 100,   // Beginner: 100ms
        6..=10 => 500,  // Intermediate: 500ms
        11..=15 => 1000, // Advanced: 1s
        _ => 2000,      // Master: 2s
    };

    engine
        .send_command(EngineCommand::Go(GoParams {
            movetime: Some(movetime),
            depth: None,
            infinite: false,
        }))
        .await?;

    Ok(cozy_chess::Move {
        from: cozy_chess::Square::new(cozy_chess::File::A, cozy_chess::Rank::First),
        to: cozy_chess::Square::new(cozy_chess::File::A, cozy_chess::Rank::Second),
        promotion: None,
    })
}
