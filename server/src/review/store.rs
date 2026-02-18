use crate::persistence::{JsonStore, PersistenceError};
use std::path::PathBuf;

use super::types::GameReview;

/// Persistence layer for game reviews. Uses JSON files in a directory.
pub struct ReviewStore {
    inner: JsonStore<GameReview>,
}

impl ReviewStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let dir = data_dir.join("reviews");
        Self {
            inner: JsonStore::new(dir),
        }
    }

    /// Save a review (partial or complete).
    pub fn save(&self, review: &GameReview) -> Result<(), PersistenceError> {
        self.inner.save(review)?;
        Ok(())
    }

    /// Load a review by game_id.
    pub fn load(&self, game_id: &str) -> Result<Option<GameReview>, PersistenceError> {
        self.inner.load(game_id)
    }

    /// List all reviews.
    pub fn list(&self) -> Result<Vec<GameReview>, PersistenceError> {
        self.inner.load_all()
    }

    /// Delete a review by game_id.
    pub fn delete(&self, game_id: &str) -> Result<(), PersistenceError> {
        self.inner.delete(game_id)
    }
}

#[cfg(test)]
impl ReviewStore {
    fn new_in(dir: PathBuf) -> Self {
        Self {
            inner: JsonStore::new(dir),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::types::{AnalysisScore, MoveClassification, PositionReview, ReviewStatus};

    fn sample_review(game_id: &str, status: ReviewStatus) -> GameReview {
        GameReview {
            game_id: game_id.to_string(),
            status,
            positions: vec![PositionReview {
                ply: 0,
                fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
                played_san: "e4".to_string(),
                best_move_san: "e4".to_string(),
                best_move_uci: "e2e4".to_string(),
                eval_before: AnalysisScore::Centipawns(20),
                eval_after: AnalysisScore::Centipawns(-25),
                eval_best: AnalysisScore::Centipawns(20),
                classification: MoveClassification::Best,
                cp_loss: 0,
                pv: vec!["e2e4".to_string()],
                depth: 18,
                clock_ms: None,
            }],
            white_accuracy: Some(95.0),
            black_accuracy: None,
            total_plies: 10,
            analyzed_plies: 1,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: None,
            winner: Some("White".to_string()),
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::new_in(dir.path().join("reviews"));
        let review = sample_review("game_1", ReviewStatus::Complete);
        store.save(&review).unwrap();
        let loaded = store.load("game_1").unwrap().unwrap();
        assert_eq!(loaded.game_id, "game_1");
        assert_eq!(loaded.positions.len(), 1);
        assert_eq!(loaded.status, ReviewStatus::Complete);
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::new_in(dir.path().join("reviews"));
        assert!(store.load("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_load_after_save() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::new_in(dir.path().join("reviews"));
        assert!(store.load("game_1").unwrap().is_none());
        store
            .save(&sample_review("game_1", ReviewStatus::Complete))
            .unwrap();
        assert!(store.load("game_1").unwrap().is_some());
    }

    #[test]
    fn test_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::new_in(dir.path().join("reviews"));
        store
            .save(&sample_review("game_1", ReviewStatus::Complete))
            .unwrap();
        store.delete("game_1").unwrap();
        assert!(store.load("game_1").unwrap().is_none());
    }

    #[test]
    fn test_partial_review_persist() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::new_in(dir.path().join("reviews"));

        let review = sample_review(
            "game_1",
            ReviewStatus::Analyzing {
                current_ply: 5,
                total_plies: 20,
            },
        );
        store.save(&review).unwrap();

        let loaded = store.load("game_1").unwrap().unwrap();
        assert_eq!(
            loaded.status,
            ReviewStatus::Analyzing {
                current_ply: 5,
                total_plies: 20
            }
        );
    }
}
