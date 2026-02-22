//! Async repository trait definitions for the persistence layer.
//!
//! Each trait abstracts over a specific domain aggregate, allowing both
//! JSON-file and SQLite backends to be used interchangeably via static
//! dispatch (generic manager types).
//!
//! Methods return `impl Future + Send` rather than using `async fn` so that
//! the futures are guaranteed `Send` — required by tonic's `#[async_trait]`
//! and `tokio::spawn`.

use super::{FinishedGameData, PersistenceError, SavedPositionData, SuspendedSessionData};
use analysis::{AdvancedGameAnalysis, GameReview};
use std::future::Future;

/// Repository for suspended chess sessions.
pub trait SessionRepository: Send + Sync {
    fn save_session(
        &self,
        data: &SuspendedSessionData,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<SuspendedSessionData>, PersistenceError>> + Send;
    fn load_session(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<SuspendedSessionData>, PersistenceError>> + Send;
    fn delete_session(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}

/// Repository for saved board positions.
///
/// Implementations must enforce the default-position protection invariant:
/// positions with `is_default == true` must not be deletable.
pub trait PositionRepository: Send + Sync {
    fn save_position(
        &self,
        data: &SavedPositionData,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
    fn list_positions(
        &self,
    ) -> impl Future<Output = Result<Vec<SavedPositionData>, PersistenceError>> + Send;
    fn delete_position(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}

/// Repository for completed games with their move history.
///
/// Implementations must store moves atomically with the game record.
/// For SQLite, this means a transaction spanning `finished_games` and
/// `stored_moves` tables; the nested `Vec<StoredMoveRecord>` is an
/// implementation detail hidden from callers.
pub trait FinishedGameRepository: Send + Sync {
    fn save_game(
        &self,
        data: &FinishedGameData,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
    fn list_games(
        &self,
    ) -> impl Future<Output = Result<Vec<FinishedGameData>, PersistenceError>> + Send;
    fn load_game(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<FinishedGameData>, PersistenceError>> + Send;
    fn delete_game(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}

/// Repository for engine-analysis game reviews.
///
/// Stores both the review header and per-position analysis data.
/// Implementations must guarantee atomic save/load of the full aggregate.
pub trait ReviewRepository: Send + Sync {
    fn save_review(
        &self,
        review: &GameReview,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
    fn load_review(
        &self,
        game_id: &str,
    ) -> impl Future<Output = Result<Option<GameReview>, PersistenceError>> + Send;
    fn list_reviews(
        &self,
    ) -> impl Future<Output = Result<Vec<GameReview>, PersistenceError>> + Send;
    fn delete_review(
        &self,
        game_id: &str,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}

/// Repository for advanced game analyses (tension, king safety, tactics).
///
/// Stores the analysis header along with per-position metrics and
/// psychological profiles. Implementations must guarantee atomicity.
pub trait AdvancedAnalysisRepository: Send + Sync {
    fn save_analysis(
        &self,
        analysis: &AdvancedGameAnalysis,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
    fn load_analysis(
        &self,
        game_id: &str,
    ) -> impl Future<Output = Result<Option<AdvancedGameAnalysis>, PersistenceError>> + Send;
    fn delete_analysis(
        &self,
        game_id: &str,
    ) -> impl Future<Output = Result<(), PersistenceError>> + Send;
}
