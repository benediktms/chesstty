//! Persistence layer for chesstty.
//!
//! ## Architecture
//!
//! The persistence layer is built around async repository traits defined in
//! [`traits`]. Each trait abstracts over a domain aggregate (sessions, positions,
//! finished games, reviews, advanced analyses).
//!
//! **Production backend**: SQLite via `sqlx` (see [`sqlite`]). A single database
//! file holds nine STRICT tables with foreign-key constraints and WAL mode for
//! concurrent reads. The [`sqlite::Database`] type owns the connection pool and
//! runs embedded migrations on startup.
//!
//! **Test backend**: The original JSON-file stores (`SessionStore`, `PositionStore`,
//! `FinishedGameStore`) are still compiled under `#[cfg(test)]`. They satisfy the
//! same traits, keeping unit tests fast and filesystem-isolated.
//!
//! **Data migration**: [`sqlite::migrate_json_to_sqlite`] is called once on startup.
//! It reads any existing JSON records from the legacy data directory and inserts
//! them into SQLite, then leaves the JSON files in place as a backup. The migration
//! is idempotent — re-running it on already-migrated data is safe.
//!
//! ## Manager generics
//!
//! `SessionManager<D>` and `ReviewManager<D>` are generic over `D: Persistence`.
//! Concrete type parameters are resolved in `main.rs`, keeping the managers
//! independent of any specific backend.

mod finished_game_store;
mod json_store;
mod position_store;
mod session_store;

pub mod sqlite;
pub mod traits;

pub(crate) use json_store::{JsonStore, Storable};
pub use traits::{
    AdvancedAnalysisRepository, FinishedGameRepository, Persistence, PositionRepository,
    ReviewRepository, SessionRepository,
};

pub use finished_game_store::{FinishedGameData, StoredMoveRecord};
pub use position_store::SavedPositionData;
pub use session_store::SuspendedSessionData;

#[cfg(test)]
pub use finished_game_store::FinishedGameStore;
#[cfg(test)]
pub use position_store::PositionStore;
#[cfg(test)]
pub use session_store::SessionStore;

/// Test persistence provider backed by JSON file stores.
#[cfg(test)]
pub struct JsonPersistence;

#[cfg(test)]
impl Persistence for JsonPersistence {
    type Sessions = SessionStore;
    type Positions = PositionStore;
    type FinishedGames = FinishedGameStore;
    type Reviews = crate::review::store::ReviewStore;
    type Advanced = crate::review::advanced::store::AdvancedAnalysisStore;
}

use std::time::{SystemTime, UNIX_EPOCH};

/// Errors from the persistence layer.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Cannot delete default positions")]
    DefaultPositionProtected,
    #[error("Migration error: {0}")]
    Migration(String),
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
