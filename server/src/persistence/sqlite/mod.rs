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

mod database;
mod session_repo;
mod position_repo;
mod finished_game_repo;
mod review_repo;
mod advanced_repo;
mod migrate_json;
#[cfg(test)]
mod integration_tests;
pub(crate) mod helpers;

pub use database::Database;
pub use session_repo::SqliteSessionRepository;
pub use position_repo::SqlitePositionRepository;
pub use finished_game_repo::SqliteFinishedGameRepository;
pub use review_repo::SqliteReviewRepository;
pub use advanced_repo::SqliteAdvancedAnalysisRepository;
pub use migrate_json::migrate_json_to_sqlite;
