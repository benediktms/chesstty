use chess::{EngineAnalysis, GameMode, GamePhase};

use super::commands::EngineConfig;

/// Complete, immutable snapshot of session state.
/// Sent to clients on every state change and on subscribe.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub fen: String,
    pub side_to_move: String,
    pub phase: GamePhase,
    pub game_mode: GameMode,
    pub status: cozy_chess::GameStatus,
    pub move_count: usize,
    pub history: Vec<MoveRecord>,
    pub last_move: Option<(String, String)>,
    pub engine_config: Option<EngineConfig>,
    pub analysis: Option<EngineAnalysis>,
    pub engine_thinking: bool,
    pub timer: Option<TimerSnapshot>,
}

/// A single move in the history.
#[derive(Debug, Clone)]
pub struct MoveRecord {
    pub from: String,
    pub to: String,
    pub piece: String,
    pub captured: Option<String>,
    pub promotion: Option<String>,
    pub san: String,
    pub fen_after: String,
}

/// Timer state for the client to render.
#[derive(Debug, Clone)]
pub struct TimerSnapshot {
    pub white_remaining_ms: u64,
    pub black_remaining_ms: u64,
    pub active_side: Option<String>, // "white", "black", or None
}
