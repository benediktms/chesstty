use crate::uci::{format_uci_move, parse_uci_message, UciMessage};
use crate::{EngineCommand, EngineEvent, GoParams, UciMessageDirection};
use cozy_chess::Move;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;

pub struct StockfishEngine {
    process: Child,
    stdin: mpsc::Sender<String>,
    command_tx: mpsc::Sender<EngineCommand>,
    event_rx: mpsc::Receiver<EngineEvent>,
}

/// Configuration for engine performance tuning.
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    pub skill_level: Option<u8>,
    pub threads: Option<u32>,
    pub hash_mb: Option<u32>,
}

impl StockfishEngine {
    /// Spawn a new Stockfish instance with just skill level (backwards compatible).
    pub async fn spawn(skill_level: Option<u8>) -> Result<Self, String> {
        Self::spawn_with_config(EngineConfig {
            skill_level,
            ..Default::default()
        })
        .await
    }

    /// Spawn a new Stockfish instance with full configuration.
    #[tracing::instrument(level = "info")]
    pub async fn spawn_with_config(config: EngineConfig) -> Result<Self, String> {
        let skill_level = config.skill_level;
        tracing::info!("Starting Stockfish engine spawn (config: {:?})", config);
        let path = find_stockfish_path().ok_or("Stockfish not found")?;
        tracing::info!("Found Stockfish at: {:?}", path);

        tracing::debug!("Spawning Stockfish process");
        let mut process = tokio::process::Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                tracing::error!("Failed to spawn Stockfish: {}", e);
                format!("Failed to spawn Stockfish: {}", e)
            })?;

        tracing::debug!("Stockfish process spawned, getting stdin/stdout");
        let mut stdin = process.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = process.stdout.take().ok_or("Failed to get stdout")?;

        // Initialize UCI
        tracing::debug!("Sending 'uci' command");
        stdin.write_all(b"uci\n").await.map_err(|e| {
            tracing::error!("Failed to write 'uci' to stdin: {}", e);
            format!("Failed to write to stdin: {}", e)
        })?;
        stdin.flush().await.map_err(|e| {
            tracing::error!("Failed to flush stdin: {}", e);
            format!("Failed to flush: {}", e)
        })?;
        tracing::debug!("'uci' command sent successfully");

        // Create channels for communication
        let (command_tx, mut command_rx) = mpsc::channel::<EngineCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<EngineEvent>(32);

        // Spawn output reader task
        tracing::debug!("Spawning output reader task");
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        tracing::warn!("Stockfish stdout EOF - engine closed");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        tracing::trace!("UCI << {}", trimmed);

                        // Emit raw UCI message event
                        let _ = event_tx_clone
                            .send(EngineEvent::RawUciMessage {
                                direction: UciMessageDirection::FromEngine,
                                message: trimmed.to_string(),
                            })
                            .await;

                        if let Ok(msg) = parse_uci_message(trimmed) {
                            let event = match msg {
                                UciMessage::UciOk => {
                                    tracing::debug!("Received uciok");
                                    EngineEvent::Ready
                                }
                                UciMessage::ReadyOk => {
                                    tracing::debug!("Received readyok");
                                    EngineEvent::Ready
                                }
                                UciMessage::BestMove { mv, .. } => {
                                    tracing::info!("Received bestmove: {:?}", mv);
                                    EngineEvent::BestMove(mv)
                                }
                                UciMessage::Info(info) => {
                                    tracing::trace!("Received info: {:?}", info);
                                    EngineEvent::Info(info)
                                }
                                _ => {
                                    tracing::trace!("Ignoring UCI message: {:?}", msg);
                                    continue;
                                }
                            };

                            if let Err(e) = event_tx_clone.send(event).await {
                                tracing::error!("Failed to send event to channel: {}", e);
                            }
                        } else {
                            tracing::trace!("Failed to parse UCI message: {}", trimmed);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading from Stockfish stdout: {}", e);
                        break;
                    }
                }
            }
            tracing::info!("Output reader task exiting");
        });

        // Wait for uciok
        tracing::debug!("Waiting for uciok from engine");
        let mut temp_rx = event_rx;
        let wait_result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Some(event) = temp_rx.recv().await {
                if matches!(event, EngineEvent::Ready) {
                    tracing::debug!("Received uciok, engine ready");
                    return Ok(());
                }
            }
            Err("Engine closed before sending uciok")
        })
        .await;

        match wait_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("Engine initialization failed: {}", e);
                return Err(format!("Engine initialization failed: {}", e));
            }
            Err(_) => {
                tracing::error!("Timeout waiting for uciok");
                return Err("Timeout waiting for engine to respond".to_string());
            }
        }
        let mut event_rx = temp_rx;

        // Set skill level if provided
        if let Some(level) = skill_level {
            tracing::info!("Setting skill level to {}", level);
            stdin
                .write_all(format!("setoption name Skill Level value {}\n", level).as_bytes())
                .await
                .map_err(|e| {
                    tracing::error!("Failed to set skill level: {}", e);
                    format!("Failed to set skill level: {}", e)
                })?;
            stdin.flush().await.map_err(|e| {
                tracing::error!("Failed to flush after skill level: {}", e);
                format!("Failed to flush: {}", e)
            })?;
            tracing::debug!("Skill level set successfully");
        }

        // Set Threads if provided
        if let Some(threads) = config.threads {
            let threads = threads.clamp(1, 16);
            tracing::info!("Setting Threads to {}", threads);
            stdin
                .write_all(format!("setoption name Threads value {}\n", threads).as_bytes())
                .await
                .map_err(|e| format!("Failed to set Threads: {}", e))?;
            stdin.flush().await.map_err(|e| format!("Failed to flush: {}", e))?;
        }

        // Set Hash if provided
        if let Some(hash_mb) = config.hash_mb {
            let hash_mb = hash_mb.clamp(1, 2048);
            tracing::info!("Setting Hash to {} MB", hash_mb);
            stdin
                .write_all(format!("setoption name Hash value {}\n", hash_mb).as_bytes())
                .await
                .map_err(|e| format!("Failed to set Hash: {}", e))?;
            stdin.flush().await.map_err(|e| format!("Failed to flush: {}", e))?;
        }

        // Clone stdin for the command processor task
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);

        // Spawn stdin writer task
        tracing::debug!("Spawning stdin writer task");
        let event_tx_for_stdin = event_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = stdin_rx.recv().await {
                let trimmed = cmd.trim();
                tracing::trace!("UCI >> {}", trimmed);

                // Emit raw UCI message event
                let _ = event_tx_for_stdin
                    .send(EngineEvent::RawUciMessage {
                        direction: UciMessageDirection::ToEngine,
                        message: trimmed.to_string(),
                    })
                    .await;

                if let Err(e) = stdin.write_all(cmd.as_bytes()).await {
                    tracing::error!("Failed to write to stdin: {}", e);
                }
                if let Err(e) = stdin.flush().await {
                    tracing::error!("Failed to flush stdin: {}", e);
                }
            }
            tracing::info!("Stdin writer task exiting");
        });

        // Send isready
        tracing::debug!("Sending 'isready' command");
        let _ = stdin_tx.send("isready\n".to_string()).await;

        // Spawn command processor task
        tracing::debug!("Spawning command processor task");
        let event_tx_for_commands = event_tx.clone();
        let stdin_tx_for_commands = stdin_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                tracing::debug!("Processing engine command: {:?}", cmd);
                let cmd_str = match cmd {
                    EngineCommand::SetPosition { ref fen, ref moves } => {
                        let mut position_cmd = format!("position fen {}", fen);
                        if !moves.is_empty() {
                            position_cmd.push_str(" moves");
                            for mv in moves {
                                position_cmd.push_str(&format!(" {}", format_uci_move(&mv)));
                            }
                        }
                        position_cmd.push('\n');
                        tracing::info!("Setting position: FEN={}, moves={}", fen, moves.len());
                        position_cmd
                    }
                    EngineCommand::SetOption { name, value } => {
                        let cmd = if let Some(val) = value {
                            format!("setoption name {} value {}\n", name, val)
                        } else {
                            format!("setoption name {}\n", name)
                        };
                        tracing::info!("Setting option: {}", cmd.trim());
                        cmd
                    }
                    EngineCommand::Go(params) => {
                        let mut go_cmd = "go".to_string();
                        if let Some(movetime) = params.movetime {
                            go_cmd.push_str(&format!(" movetime {}", movetime));
                            tracing::info!(
                                "Starting engine calculation with movetime={}ms",
                                movetime
                            );
                        } else if let Some(depth) = params.depth {
                            go_cmd.push_str(&format!(" depth {}", depth));
                            tracing::info!("Starting engine calculation with depth={}", depth);
                        } else if params.infinite {
                            go_cmd.push_str(" infinite");
                            tracing::info!("Starting engine calculation in infinite mode");
                        } else {
                            go_cmd.push_str(" movetime 1000"); // Default 1 second
                            tracing::info!(
                                "Starting engine calculation with default movetime=1000ms"
                            );
                        }
                        go_cmd.push('\n');
                        go_cmd
                    }
                    EngineCommand::Stop => {
                        tracing::info!("Sending stop command to engine");
                        "stop\n".to_string()
                    }
                    EngineCommand::Quit => {
                        tracing::info!("Sending quit command to engine");
                        let _ = stdin_tx_for_commands.send("quit\n".to_string()).await;
                        break;
                    }
                };

                if let Err(e) = stdin_tx_for_commands.send(cmd_str).await {
                    tracing::error!("Failed to send command to stdin channel: {}", e);
                }
            }
            tracing::info!("Command processor task exiting");
        });

        tracing::info!("Stockfish engine spawned and initialized successfully");
        Ok(Self {
            process,
            stdin: stdin_tx,
            command_tx,
            event_rx,
        })
    }

    /// Send a command to the engine
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn send_command(&self, cmd: EngineCommand) -> Result<(), String> {
        tracing::debug!("Queueing command: {:?}", cmd);
        self.command_tx.send(cmd).await.map_err(|e| {
            tracing::error!("Failed to send command to queue: {}", e);
            format!("Failed to send command: {}", e)
        })
    }

    /// Try to receive an event from the engine (non-blocking)
    pub fn try_recv_event(&mut self) -> Option<EngineEvent> {
        match self.event_rx.try_recv().ok() {
            Some(event) => {
                tracing::trace!("Received event: {:?}", event);
                Some(event)
            }
            None => None,
        }
    }

    /// Receive an event from the engine (blocking)
    pub async fn recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx.recv().await
    }

    /// Shutdown the engine
    pub async fn shutdown(mut self) {
        let _ = self.send_command(EngineCommand::Quit).await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), self.process.wait()).await;
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
        0..=5 => 100,    // Beginner: 100ms
        6..=10 => 500,   // Intermediate: 500ms
        11..=15 => 1000, // Advanced: 1s
        _ => 2000,       // Master: 2s
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
