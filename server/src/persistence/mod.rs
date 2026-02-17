mod finished_game_store;
mod json_store;
mod position_store;
mod session_store;

pub(crate) use json_store::{JsonStore, Storable};

pub use finished_game_store::{FinishedGameData, FinishedGameStore, StoredMoveRecord};
pub use position_store::{PositionStore, SavedPositionData};
pub use session_store::{SessionStore, SuspendedSessionData};

use std::time::{SystemTime, UNIX_EPOCH};

/// Errors from the persistence layer.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Cannot delete default positions")]
    DefaultPositionProtected,
}

/// Generate a unique suspended session ID using timestamp + random suffix.
pub fn generate_suspended_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("session_{}", ts)
}

/// Generate a unique finished game ID using timestamp.
pub fn generate_finished_game_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("game_{}", ts)
}

/// Generate a unique position ID.
pub fn generate_position_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("pos_{}", ts)
}

/// Get the current unix timestamp in seconds.
pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
