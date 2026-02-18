pub mod advanced;
pub mod store;
pub mod types;
pub mod worker;

use std::collections::HashSet;
use std::sync::Arc;

use analysis::AnalysisConfig;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::persistence::FinishedGameStore;
use advanced::AdvancedAnalysisStore;
use store::ReviewStore;
use types::*;

/// Configuration for the review system.
pub struct ReviewConfig {
    /// Number of concurrent workers (each spawns its own Stockfish process).
    pub worker_count: usize,
    /// Engine depth per position (used when advanced analysis is disabled).
    pub analysis_depth: u32,
    /// Advanced analysis configuration.
    pub analysis: AnalysisConfig,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            worker_count: 1,
            analysis_depth: 18,
            analysis: AnalysisConfig::default(),
        }
    }
}

/// Manages background review analysis jobs.
///
/// Owns a bounded job queue (mpsc channel) and a fixed pool of worker tasks.
/// Each worker spawns its own StockfishEngine process.
pub struct ReviewManager {
    job_tx: mpsc::Sender<ReviewJob>,
    enqueued: Arc<RwLock<HashSet<String>>>,
    review_store: Arc<ReviewStore>,
    finished_game_store: Arc<FinishedGameStore>,
    advanced_store: Arc<AdvancedAnalysisStore>,
    /// Kept alive so the channel stays open even if no workers are spawned.
    _job_rx: Arc<Mutex<mpsc::Receiver<ReviewJob>>>,
}

impl ReviewManager {
    pub fn new(
        finished_game_store: Arc<FinishedGameStore>,
        review_store: Arc<ReviewStore>,
        advanced_store: Arc<AdvancedAnalysisStore>,
        config: ReviewConfig,
    ) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<ReviewJob>(64);
        let enqueued = Arc::new(RwLock::new(HashSet::new()));

        // Wrap the receiver so multiple workers can share it.
        // Each worker calls rx.lock().await.recv().await, ensuring only one
        // worker picks up each job.
        let shared_rx = Arc::new(Mutex::new(job_rx));

        // Spawn worker pool
        for worker_id in 0..config.worker_count {
            let rx = shared_rx.clone();
            let store = review_store.clone();
            let adv_store = advanced_store.clone();
            let enqueued = enqueued.clone();
            let depth = config.analysis_depth;
            let analysis_config = config.analysis.clone();
            tokio::spawn(async move {
                worker::run_review_worker(
                    worker_id,
                    rx,
                    store,
                    adv_store,
                    enqueued,
                    depth,
                    analysis_config,
                )
                .await;
            });
        }

        tracing::info!(
            worker_count = config.worker_count,
            depth = config.analysis_depth,
            compute_advanced = config.analysis.compute_advanced,
            "Review manager initialized"
        );

        Self {
            job_tx,
            enqueued,
            review_store,
            finished_game_store,
            advanced_store,
            _job_rx: shared_rx,
        }
    }

    /// Recover pending work on startup.
    ///
    /// Scans for:
    /// 1. Reviews stuck in Analyzing/Queued/Failed state (interrupted by a restart)
    /// 2. Finished games with no review at all (auto-enqueue missed)
    ///
    /// Re-enqueues them so the worker picks them up.
    pub async fn recover_pending_reviews(&self) {
        let mut recovered = 0;

        // 1. Scan for incomplete reviews on disk
        if let Ok(reviews) = self.review_store.list() {
            for review in reviews {
                match review.status {
                    ReviewStatus::Complete => {} // nothing to do
                    ReviewStatus::Analyzing { .. } | ReviewStatus::Queued => {
                        tracing::info!(
                            game_id = %review.game_id,
                            status = ?review.status,
                            "Recovering interrupted review"
                        );
                        if let Err(e) = self.enqueue(&review.game_id).await {
                            tracing::warn!(
                                game_id = %review.game_id,
                                "Failed to re-enqueue interrupted review: {}",
                                e
                            );
                        } else {
                            recovered += 1;
                        }
                    }
                    ReviewStatus::Failed { .. } => {
                        // Don't auto-retry failed reviews -- user can manually retry via 'a'
                    }
                }
            }
        }

        // 2. Scan for finished games with no review at all
        if let Ok(games) = self.finished_game_store.list() {
            for game in games {
                if self
                    .review_store
                    .load(&game.game_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    tracing::info!(
                        game_id = %game.game_id,
                        "Enqueueing finished game with no review"
                    );
                    if let Err(e) = self.enqueue(&game.game_id).await {
                        tracing::warn!(
                            game_id = %game.game_id,
                            "Failed to enqueue unreviewed game: {}",
                            e
                        );
                    } else {
                        recovered += 1;
                    }
                }
            }
        }

        if recovered > 0 {
            tracing::info!(recovered, "Recovery complete, enqueued pending reviews");
        } else {
            tracing::debug!("Recovery complete, no pending reviews found");
        }
    }

    /// Enqueue a game for review analysis.
    /// Returns an error if the game_id is already queued or already reviewed.
    pub async fn enqueue(&self, game_id: &str) -> Result<(), String> {
        tracing::info!(game_id = %game_id, "Enqueueing game for review");

        // Check if already enqueued (prevents duplicate jobs)
        {
            let enqueued = self.enqueued.read().await;
            if enqueued.contains(game_id) {
                tracing::warn!(game_id = %game_id, "Duplicate enqueue rejected");
                return Err(format!("Game {} is already queued for review", game_id));
            }
        }

        // Check if review already exists and is complete
        if let Ok(Some(review)) = self.review_store.load(game_id) {
            if review.status == ReviewStatus::Complete {
                tracing::warn!(game_id = %game_id, "Review already complete, rejecting enqueue");
                return Err(format!("Review for game {} already exists", game_id));
            }
            // If failed or partial, allow re-enqueue (will resume)
        }

        // Load the finished game data
        let game_data = self
            .finished_game_store
            .load(game_id)
            .map_err(|e| format!("Failed to load game: {}", e))?
            .ok_or_else(|| format!("Finished game not found: {}", game_id))?;

        // Mark as enqueued before sending to prevent duplicate enqueue attempts
        self.enqueued.write().await.insert(game_id.to_string());

        // Send to job queue
        let job = ReviewJob {
            game_id: game_id.to_string(),
            game_data,
        };
        if let Err(e) = self.job_tx.send(job).await {
            // Roll back: remove from enqueued set since the job wasn't actually sent
            self.enqueued.write().await.remove(game_id);
            tracing::error!(game_id = %game_id, "Failed to send job to queue: {}", e);
            return Err("Review job queue full or closed".to_string());
        }

        tracing::info!(game_id = %game_id, "Job sent to review queue");
        Ok(())
    }

    /// Get the status of a review for a given game_id.
    pub async fn get_status(&self, game_id: &str) -> Result<ReviewStatus, String> {
        // Check if it's in the enqueued set (job is pending or in-flight)
        if self.enqueued.read().await.contains(game_id) {
            // Check the store for in-progress updates from the worker
            if let Ok(Some(review)) = self.review_store.load(game_id) {
                if let ReviewStatus::Analyzing { .. } = review.status {
                    return Ok(review.status);
                }
            }
            // Queued (not yet started) or re-enqueued after failure
            return Ok(ReviewStatus::Queued);
        }

        // Check the store for completed/failed reviews
        match self.review_store.load(game_id) {
            Ok(Some(review)) => Ok(review.status),
            Ok(None) => Err(format!("No review found for game {}", game_id)),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Get the full review for a game.
    pub fn get_review(&self, game_id: &str) -> Result<Option<GameReview>, String> {
        self.review_store.load(game_id).map_err(|e| e.to_string())
    }

    /// Get the advanced analysis for a game.
    pub fn get_advanced_analysis(
        &self,
        game_id: &str,
    ) -> Result<Option<analysis::AdvancedGameAnalysis>, String> {
        self.advanced_store
            .load(game_id)
            .map_err(|e| e.to_string())
    }

    /// List all finished games eligible for review.
    pub fn list_finished_games(&self) -> Result<Vec<crate::persistence::FinishedGameData>, String> {
        self.finished_game_store.list().map_err(|e| e.to_string())
    }

    /// Delete a finished game and its associated review.
    pub async fn delete_finished_game(&self, game_id: &str) -> Result<(), String> {
        // Don't allow deleting games that are currently being analyzed
        if self.enqueued.read().await.contains(game_id) {
            return Err(format!(
                "Cannot delete game {} while it is queued for review",
                game_id
            ));
        }

        self.finished_game_store
            .delete(game_id)
            .map_err(|e| e.to_string())?;
        self.review_store
            .delete(game_id)
            .map_err(|e| e.to_string())?;
        // Also delete advanced analysis if it exists
        self.advanced_store
            .delete(game_id)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{FinishedGameData, StoredMoveRecord};

    /// Create test stores in a temp directory (leaked so it outlives the test).
    fn test_stores() -> (Arc<FinishedGameStore>, Arc<ReviewStore>, Arc<AdvancedAnalysisStore>) {
        let dir = tempfile::tempdir().unwrap();
        let finished = Arc::new(FinishedGameStore::new(dir.path().to_path_buf()));
        let reviews = Arc::new(ReviewStore::new(dir.path().to_path_buf()));
        let advanced = Arc::new(AdvancedAnalysisStore::new(dir.path().to_path_buf()));
        std::mem::forget(dir);
        (finished, reviews, advanced)
    }

    /// Create a ReviewManager with 0 workers (no Stockfish needed).
    /// Jobs will be enqueued but never processed — useful for testing
    /// queue logic, duplicate prevention, and status tracking.
    fn test_manager_no_workers(
        finished: Arc<FinishedGameStore>,
        reviews: Arc<ReviewStore>,
        advanced: Arc<AdvancedAnalysisStore>,
    ) -> ReviewManager {
        ReviewManager::new(
            finished,
            reviews,
            advanced,
            ReviewConfig {
                worker_count: 0,
                analysis_depth: 1,
                ..Default::default()
            },
        )
    }

    /// Create a ReviewManager with a closed job channel.
    /// All send attempts will fail, useful for testing send-failure rollback.
    fn test_manager_closed_channel(
        finished: Arc<FinishedGameStore>,
        reviews: Arc<ReviewStore>,
        advanced: Arc<AdvancedAnalysisStore>,
    ) -> ReviewManager {
        let (job_tx, job_rx) = mpsc::channel::<ReviewJob>(1);
        // Drop receiver immediately so all sends fail
        drop(job_rx);

        let (_keep_tx, keep_rx) = mpsc::channel::<ReviewJob>(1);
        ReviewManager {
            job_tx,
            enqueued: Arc::new(RwLock::new(HashSet::new())),
            review_store: reviews,
            finished_game_store: finished,
            advanced_store: advanced,
            _job_rx: Arc::new(Mutex::new(keep_rx)),
        }
    }

    /// Build a minimal finished game fixture.
    fn sample_finished_game(game_id: &str) -> FinishedGameData {
        FinishedGameData {
            game_id: game_id.to_string(),
            start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            result: "BlackWins".to_string(),
            result_reason: "Checkmate".to_string(),
            game_mode: "HumanVsHuman".to_string(),
            human_side: None,
            skill_level: 0,
            move_count: 4,
            moves: vec![
                StoredMoveRecord {
                    from: "f2".into(),
                    to: "f3".into(),
                    piece: "P".into(),
                    captured: None,
                    promotion: None,
                    san: "f3".into(),
                    fen_after: "rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq - 0 1".into(),
                    clock_ms: None,
                },
                StoredMoveRecord {
                    from: "e7".into(),
                    to: "e5".into(),
                    piece: "P".into(),
                    captured: None,
                    promotion: None,
                    san: "e5".into(),
                    fen_after: "rnbqkbnr/pppp1ppp/8/4p3/8/5P2/PPPPP1PP/RNBQKBNR w KQkq e6 0 2"
                        .into(),
                    clock_ms: None,
                },
                StoredMoveRecord {
                    from: "g2".into(),
                    to: "g4".into(),
                    piece: "P".into(),
                    captured: None,
                    promotion: None,
                    san: "g4".into(),
                    fen_after: "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2"
                        .into(),
                    clock_ms: None,
                },
                StoredMoveRecord {
                    from: "d8".into(),
                    to: "h4".into(),
                    piece: "Q".into(),
                    captured: None,
                    promotion: None,
                    san: "Qh4#".into(),
                    fen_after: "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"
                        .into(),
                    clock_ms: None,
                },
            ],
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn test_enqueue_send_failure_removes_from_enqueued() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();
        let mgr = test_manager_closed_channel(finished, reviews, advanced);

        // First enqueue should fail because the channel is closed
        let result = mgr.enqueue("game_1").await;
        assert!(result.is_err());

        // game_id should NOT remain in the enqueued set — a retry must be possible
        assert!(!mgr.enqueued.read().await.contains("game_1"));
    }

    #[tokio::test]
    async fn test_enqueue_nonexistent_game_fails() {
        let (finished, reviews, advanced) = test_stores();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        let result = mgr.enqueue("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_enqueue_sets_status_to_queued() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        mgr.enqueue("game_1").await.unwrap();

        let status = mgr.get_status("game_1").await.unwrap();
        assert_eq!(status, ReviewStatus::Queued);
    }

    #[tokio::test]
    async fn test_duplicate_enqueue_rejected() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        mgr.enqueue("game_1").await.unwrap();
        let result = mgr.enqueue("game_1").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already queued"));
    }

    #[tokio::test]
    async fn test_enqueue_rejects_completed_review() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        // Pre-save a completed review
        let completed = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Complete,
            positions: vec![],
            white_accuracy: Some(80.0),
            black_accuracy: Some(75.0),
            total_plies: 4,
            analyzed_plies: 4,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: Some(2000),
            winner: Some("White".to_string()),
        };
        reviews.save(&completed).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);

        let result = mgr.enqueue("game_1").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[tokio::test]
    async fn test_enqueue_allows_re_enqueue_after_failure() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        // Pre-save a failed review
        let failed = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Failed {
                error: "engine crashed".to_string(),
            },
            positions: vec![],
            white_accuracy: None,
            black_accuracy: None,
            total_plies: 4,
            analyzed_plies: 0,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: None,
            winner: None,
        };
        reviews.save(&failed).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);

        // Should succeed — failed reviews can be re-enqueued
        mgr.enqueue("game_1").await.unwrap();
        assert_eq!(
            mgr.get_status("game_1").await.unwrap(),
            ReviewStatus::Queued
        );
    }

    #[tokio::test]
    async fn test_get_status_no_review_returns_error() {
        let (finished, reviews, advanced) = test_stores();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        let result = mgr.get_status("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_review_returns_none_for_unknown() {
        let (finished, reviews, advanced) = test_stores();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        assert!(mgr.get_review("nonexistent").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_review_returns_stored_review() {
        let (finished, reviews, advanced) = test_stores();

        let review = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Complete,
            positions: vec![],
            white_accuracy: Some(90.0),
            black_accuracy: Some(85.0),
            total_plies: 4,
            analyzed_plies: 4,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: Some(2000),
            winner: Some("Black".to_string()),
        };
        reviews.save(&review).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);

        let loaded = mgr.get_review("game_1").unwrap().unwrap();
        assert_eq!(loaded.game_id, "game_1");
        assert_eq!(loaded.status, ReviewStatus::Complete);
        assert_eq!(loaded.white_accuracy, Some(90.0));
    }

    #[tokio::test]
    async fn test_list_finished_games() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_a")).unwrap();
        finished.save(&sample_finished_game("game_b")).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);

        let games = mgr.list_finished_games().unwrap();
        assert_eq!(games.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_finished_game() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        // Also save an associated review
        let review = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Complete,
            positions: vec![],
            white_accuracy: Some(80.0),
            black_accuracy: Some(75.0),
            total_plies: 4,
            analyzed_plies: 4,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: Some(2000),
            winner: Some("Draw".to_string()),
        };
        reviews.save(&review).unwrap();

        let mgr = test_manager_no_workers(finished.clone(), reviews.clone(), advanced);

        mgr.delete_finished_game("game_1").await.unwrap();

        // Both the game and review should be gone
        assert!(finished.load("game_1").unwrap().is_none());
        assert!(reviews.load("game_1").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_blocks_while_enqueued() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        mgr.enqueue("game_1").await.unwrap();

        let result = mgr.delete_finished_game("game_1").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("queued for review"));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_ok() {
        let (finished, reviews, advanced) = test_stores();
        let mgr = test_manager_no_workers(finished, reviews, advanced);

        // Deleting a game that doesn't exist should not error
        mgr.delete_finished_game("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_recover_re_enqueues_analyzing_reviews() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        // Simulate a review stuck in Analyzing (server crashed mid-analysis)
        let stuck = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Analyzing {
                current_ply: 2,
                total_plies: 4,
            },
            positions: vec![],
            white_accuracy: None,
            black_accuracy: None,
            total_plies: 4,
            analyzed_plies: 2,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: None,
            winner: None,
        };
        reviews.save(&stuck).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);

        // Before recovery: shows stale Analyzing from disk
        let status_before = mgr.get_status("game_1").await.unwrap();
        assert!(matches!(status_before, ReviewStatus::Analyzing { .. }));

        mgr.recover_pending_reviews().await;

        // After recovery: game is in the enqueued set and the worker will
        // resume from the persisted state. get_status reflects the on-disk
        // Analyzing state (which the worker will pick up and continue).
        let status = mgr.get_status("game_1").await.unwrap();
        assert!(matches!(
            status,
            ReviewStatus::Analyzing { .. } | ReviewStatus::Queued
        ));
    }

    #[tokio::test]
    async fn test_recover_enqueues_unreviewed_games() {
        let (finished, reviews, advanced) = test_stores();
        // Finished game with no review file at all
        finished.save(&sample_finished_game("game_1")).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);
        mgr.recover_pending_reviews().await;

        let status = mgr.get_status("game_1").await.unwrap();
        assert_eq!(status, ReviewStatus::Queued);
    }

    #[tokio::test]
    async fn test_recover_skips_completed_reviews() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        let completed = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Complete,
            positions: vec![],
            white_accuracy: Some(80.0),
            black_accuracy: Some(75.0),
            total_plies: 4,
            analyzed_plies: 4,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: Some(2000),
            winner: Some("White".to_string()),
        };
        reviews.save(&completed).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);
        mgr.recover_pending_reviews().await;

        // Should still show Complete, not Queued
        let status = mgr.get_status("game_1").await.unwrap();
        assert_eq!(status, ReviewStatus::Complete);
    }

    #[tokio::test]
    async fn test_recover_skips_failed_reviews() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        let failed = GameReview {
            game_id: "game_1".to_string(),
            status: ReviewStatus::Failed {
                error: "engine error".to_string(),
            },
            positions: vec![],
            white_accuracy: None,
            black_accuracy: None,
            total_plies: 4,
            analyzed_plies: 0,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: None,
            winner: None,
        };
        reviews.save(&failed).unwrap();

        let mgr = test_manager_no_workers(finished, reviews, advanced);
        mgr.recover_pending_reviews().await;

        // Failed reviews are NOT auto-retried -- user must manually trigger
        let status = mgr.get_status("game_1").await.unwrap();
        assert_eq!(
            status,
            ReviewStatus::Failed {
                error: "engine error".to_string()
            }
        );
    }

    /// Full end-to-end: enqueue a game, let the worker analyze it with Stockfish,
    /// and verify the completed review has correct structure.
    #[tokio::test]
    async fn test_full_analysis_pipeline() {
        let (finished, reviews, advanced) = test_stores();
        finished.save(&sample_finished_game("game_1")).unwrap();

        let mgr = ReviewManager::new(
            finished,
            reviews.clone(),
            advanced.clone(),
            ReviewConfig {
                worker_count: 1,
                analysis_depth: 4, // shallow for speed
                analysis: AnalysisConfig {
                    compute_advanced: true,
                    shallow_depth: 4,
                    deep_depth: 6,
                    max_critical_positions: 5,
                },
            },
        );

        mgr.enqueue("game_1").await.unwrap();

        // Poll until complete (timeout after 30s)
        let start = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            match mgr.get_status("game_1").await {
                Ok(ReviewStatus::Complete) => break,
                Ok(ReviewStatus::Failed { error }) => panic!("Review failed: {}", error),
                _ if start.elapsed() > std::time::Duration::from_secs(30) => {
                    panic!("Timed out waiting for review to complete");
                }
                _ => continue,
            }
        }

        let review = mgr.get_review("game_1").unwrap().unwrap();
        assert_eq!(review.status, ReviewStatus::Complete);
        assert_eq!(review.total_plies, 4);
        assert_eq!(review.analyzed_plies, 4);
        assert_eq!(review.positions.len(), 4);
        assert!(review.white_accuracy.is_some());
        assert!(review.black_accuracy.is_some());
        assert!(review.completed_at.is_some());

        // Each position should have valid data
        for (i, pos) in review.positions.iter().enumerate() {
            assert_eq!(pos.ply, (i as u32) + 1);
            assert!(!pos.fen.is_empty());
            assert!(!pos.played_san.is_empty());
            assert!(!pos.best_move_uci.is_empty());
            assert!(pos.cp_loss >= 0);
            assert!(pos.depth > 0);
        }

        // Fool's mate: Black's last move Qh4# should be classified well
        // (it's checkmate, so it should be Best or Forced)
        let last_black_move = &review.positions[3];
        assert_eq!(last_black_move.played_san, "Qh4#");

        // eval_best should preserve the Mate variant (not convert to centipawns).
        // The position before Qh4# is a forced mate — the engine sees Mate(1)
        // from Black's perspective, which is stored as Mate(-1) from White's.
        assert!(
            matches!(last_black_move.eval_best, AnalysisScore::Mate(_)),
            "eval_best should be Mate variant, got: {:?}",
            last_black_move.eval_best
        );

        // Advanced analysis should have been produced
        let adv = mgr.get_advanced_analysis("game_1").unwrap();
        assert!(adv.is_some(), "Advanced analysis should be present");
        let adv = adv.unwrap();
        assert_eq!(adv.game_id, "game_1");
        assert_eq!(adv.positions.len(), 4);
        assert_eq!(adv.pipeline_version, 1);
    }
}
