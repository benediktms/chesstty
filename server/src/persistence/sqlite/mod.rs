//! SQLite-backed repository implementations.
//!
//! ## Database setup
//!
//! [`Database`] wraps a `sqlx::SqlitePool` configured with:
//! - **WAL mode** — allows one writer and multiple concurrent readers.
//! - **Foreign keys enabled** — enforced at the connection level.
//! - **Embedded migrations** — `sqlx::migrate!` runs `migrations/001_initial_schema.sql`
//!   automatically when [`Database::open`] is called. The schema is idempotent.
//!
//! ## Repository types
//!
//! Each `Sqlite*Repository` holds an `Arc<SqlitePool>` and implements the
//! corresponding trait from [`crate::persistence::traits`]:
//!
//! | Type | Trait |
//! |------|-------|
//! | [`SqliteSessionRepository`] | `SessionRepository` |
//! | [`SqlitePositionRepository`] | `PositionRepository` |
//! | [`SqliteFinishedGameRepository`] | `FinishedGameRepository` |
//! | [`SqliteReviewRepository`] | `ReviewRepository` |
//! | [`SqliteAdvancedAnalysisRepository`] | `AdvancedAnalysisRepository` |
//!
//! Enum columns (game status, score classification, move classification) are stored
//! as `TEXT` and round-tripped through shared encode/decode helpers in [`helpers`].
//!
//! ## JSON migration
//!
//! [`migrate_json_to_sqlite`] performs a one-time, idempotent import of legacy
//! JSON records. It is called from `main.rs` before the service starts accepting
//! requests. Original JSON files are not deleted.

mod advanced_repo;
mod database;
mod finished_game_repo;
pub(crate) mod helpers;
#[cfg(test)]
mod integration_tests;
mod migrate_json;
mod position_repo;
mod review_repo;
mod session_repo;

pub use advanced_repo::SqliteAdvancedAnalysisRepository;
pub use database::Database;
pub use finished_game_repo::SqliteFinishedGameRepository;
pub use migrate_json::migrate_json_to_sqlite;
pub use position_repo::SqlitePositionRepository;
pub use review_repo::SqliteReviewRepository;
pub use session_repo::SqliteSessionRepository;

/// Production persistence provider backed by SQLite.
///
/// Implements [`super::Persistence`] by mapping each associated type to the
/// corresponding `Sqlite*Repository`.
pub struct SqlitePersistence;

impl crate::persistence::Persistence for SqlitePersistence {
    type Sessions = SqliteSessionRepository;
    type Positions = SqlitePositionRepository;
    type FinishedGames = SqliteFinishedGameRepository;
    type Reviews = SqliteReviewRepository;
    type Advanced = SqliteAdvancedAnalysisRepository;
}
