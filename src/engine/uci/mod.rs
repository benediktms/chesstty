pub mod parser;

pub use parser::{format_uci_move, parse_uci_message, parse_uci_move, UciMessage};

use crate::engine::{EngineCommand, EngineEvent, EngineInfo, Score};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

/// UCI chess engine wrapper
pub struct UciEngine {
    process: Child,
    stdin: ChildStdin,
}

impl UciEngine {
    /// Spawn a new UCI engine from the given path
    pub async fn spawn(path: &Path) -> Result<Self, UciError> {
        let mut process = tokio::process::Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = process.stdin.take().ok_or(UciError::NoStdin)?;
        let stdout = process.stdout.take().ok_or(UciError::NoStdout)?;

        let mut engine = Self { process, stdin };

        // Initialize UCI
        engine.send_command("uci").await?;

        // Spawn output reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            Self::read_output_loop(reader).await;
        });

        Ok(engine)
    }

    /// Send a UCI command to the engine
    async fn send_command(&mut self, cmd: &str) -> Result<(), UciError> {
        self.stdin.write_all(cmd.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Read and parse output from the engine
    async fn read_output_loop(mut reader: BufReader<ChildStdout>) {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if let Ok(msg) = parse_uci_message(&line) {
                        // TODO: Send to event channel
                        println!("Engine: {:?}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from engine: {}", e);
                    break;
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UciError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Engine has no stdin")]
    NoStdin,
    #[error("Engine has no stdout")]
    NoStdout,
    #[error("Malformed UCI message: {0}")]
    MalformedMessage(String),
    #[error("Unknown UCI message: {0}")]
    UnknownMessage(String),
    #[error("Invalid move: {0}")]
    InvalidMove(String),
    #[error("Invalid square: {0}")]
    InvalidSquare(String),
    #[error("Invalid promotion: {0}")]
    InvalidPromotion(String),
}
