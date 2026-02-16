use chess::EngineAnalysis;

use super::snapshot::SessionSnapshot;

/// Events broadcast from the session actor to all subscribers.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Full state snapshot after any mutation.
    StateChanged(SessionSnapshot),
    /// Transient engine analysis (frequent, lightweight).
    EngineThinking(EngineAnalysis),
    /// UCI debug log entry.
    UciMessage(UciLogEntry),
    /// Error notification.
    Error(String),
}

#[derive(Debug, Clone)]
pub struct UciLogEntry {
    pub direction: UciDirection,
    pub message: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UciDirection {
    ToEngine,
    FromEngine,
}
