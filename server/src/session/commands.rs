use chess::PlayerSide;
use cozy_chess::{Move, Square};
use tokio::sync::{broadcast, oneshot};

use super::events::SessionEvent;
use super::snapshot::SessionSnapshot;

#[derive(Debug, Clone, thiserror::Error)]
pub enum SessionError {
    #[error("Illegal move: {0}")]
    IllegalMove(String),
    #[error("Invalid FEN: {0}")]
    InvalidFen(String),
    #[error("Engine not configured")]
    EngineNotConfigured,
    #[error("Game is not ongoing")]
    GameNotOngoing,
    #[error("Nothing to undo")]
    NothingToUndo,
    #[error("Nothing to redo")]
    NothingToRedo,
    #[error("Invalid phase transition: {0}")]
    InvalidPhaseTransition(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub enabled: bool,
    pub skill_level: u8,
    pub threads: Option<u32>,
    pub hash_mb: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct LegalMove {
    pub from: String,
    pub to: String,
    pub promotion: Option<String>,
    pub san: String,
    pub is_capture: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
}

/// Commands sent to the session actor. Each embeds a oneshot for the reply.
/// Note: No TriggerEngineMove — the actor auto-triggers engine moves.
pub enum SessionCommand {
    MakeMove {
        mv: Move,
        reply: oneshot::Sender<Result<SessionSnapshot, SessionError>>,
    },
    Undo {
        reply: oneshot::Sender<Result<SessionSnapshot, SessionError>>,
    },
    Redo {
        reply: oneshot::Sender<Result<SessionSnapshot, SessionError>>,
    },
    Reset {
        fen: Option<String>,
        reply: oneshot::Sender<Result<SessionSnapshot, SessionError>>,
    },
    ConfigureEngine {
        config: EngineConfig,
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    StopEngine {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    Pause {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    Resume {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    SetTimer {
        white_ms: u64,
        black_ms: u64,
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    GetSnapshot {
        reply: oneshot::Sender<SessionSnapshot>,
    },
    GetLegalMoves {
        from: Option<Square>,
        reply: oneshot::Sender<Vec<LegalMove>>,
    },
    Subscribe {
        reply: oneshot::Sender<(SessionSnapshot, broadcast::Receiver<SessionEvent>)>,
    },
    Shutdown,
}
