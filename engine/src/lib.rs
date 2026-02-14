pub mod stockfish;
pub mod uci;

pub use stockfish::StockfishEngine;
pub use uci::{UciEngine, UciError, UciMessage};

use cozy_chess::Move;
use tokio::sync::mpsc;

/// Handle for communicating with a chess engine
pub struct EngineHandle {
    pub tx: mpsc::Sender<EngineCommand>,
    pub rx: mpsc::Receiver<EngineEvent>,
}

/// Commands sent to the engine
#[derive(Debug, Clone)]
pub enum EngineCommand {
    SetPosition { fen: String, moves: Vec<Move> },
    SetOption { name: String, value: Option<String> },
    Go(GoParams),
    Stop,
    Quit,
}

/// Parameters for the "go" command
#[derive(Debug, Clone, Default)]
pub struct GoParams {
    pub movetime: Option<u64>, // Move time in milliseconds
    pub depth: Option<u8>,     // Search depth
    pub infinite: bool,        // Search until "stop"
}

/// Events received from the engine
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Ready,
    BestMove(Move),
    Info(EngineInfo),
    Error(String),
    RawUciMessage {
        direction: UciMessageDirection,
        message: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum UciMessageDirection {
    ToEngine,
    FromEngine,
}

/// Engine analysis information
#[derive(Debug, Clone, Default)]
pub struct EngineInfo {
    pub depth: Option<u8>,
    pub seldepth: Option<u8>,
    pub time_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub score: Option<Score>,
    pub pv: Vec<Move>, // Principal variation
    pub multipv: Option<u8>,
    pub currmove: Option<Move>,
    pub hashfull: Option<u16>,
    pub nps: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum Score {
    Centipawns(i32),
    Mate(i8), // Negative for being mated
}
