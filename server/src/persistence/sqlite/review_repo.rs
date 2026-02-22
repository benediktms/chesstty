//! SQLite-backed implementation of [`ReviewRepository`].

use sqlx::SqlitePool;

use crate::persistence::now_timestamp;
use crate::persistence::PersistenceError;
use crate::persistence::traits::ReviewRepository;
use super::helpers::{
    decode_classification, decode_score, decode_status, encode_classification, encode_score,
    encode_status,
};
use analysis::{GameReview, PositionReview};

/// SQLite implementation of [`ReviewRepository`].
pub struct SqliteReviewRepository {
    pool: SqlitePool,
}

impl SqliteReviewRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl ReviewRepository for SqliteReviewRepository {
    async fn save_review(&self, review: &GameReview) -> Result<(), PersistenceError> {
        let mut tx = self.pool.begin().await?;

        let (status_str, status_current_ply, status_total_plies, status_error) =
            encode_status(&review.status);
        let status_current_ply = status_current_ply.map(|v| v as i64);
        let status_total_plies = status_total_plies.map(|v| v as i64);
        let white_accuracy = review.white_accuracy;
        let black_accuracy = review.black_accuracy;
        let total_plies = review.total_plies as i64;
        let analyzed_plies = review.analyzed_plies as i64;
        let analysis_depth = review.analysis_depth as i64;
        let created_at = now_timestamp() as i64;
        let started_at = review.started_at.map(|v| v as i64);
        let completed_at = review.completed_at.map(|v| v as i64);

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO game_reviews
                (game_id, status, status_current_ply, status_total_plies, status_error,
                 white_accuracy, black_accuracy, total_plies, analyzed_plies, analysis_depth,
                 created_at, started_at, completed_at, winner)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&review.game_id)
        .bind(status_str)
        .bind(status_current_ply)
        .bind(status_total_plies)
        .bind(status_error)
        .bind(white_accuracy)
        .bind(black_accuracy)
        .bind(total_plies)
        .bind(analyzed_plies)
        .bind(analysis_depth)
        .bind(created_at)
        .bind(started_at)
        .bind(completed_at)
        .bind(&review.winner)
        .execute(&mut *tx)
        .await?;

        for position in &review.positions {
            let ply = position.ply as i64;
            let (eb_type, eb_val) = encode_score(&position.eval_before);
            let (ea_type, ea_val) = encode_score(&position.eval_after);
            let (ebest_type, ebest_val) = encode_score(&position.eval_best);
            let classification = encode_classification(&position.classification);
            let cp_loss = position.cp_loss as i64;
            let pv_json = serde_json::to_string(&position.pv)?;
            let depth = position.depth as i64;
            let clock_ms = position.clock_ms.map(|v| v as i64);

            sqlx::query(
                r#"
                INSERT OR IGNORE INTO position_reviews
                    (game_id, ply, fen, played_san, best_move_san, best_move_uci,
                     eval_before_type, eval_before_value,
                     eval_after_type, eval_after_value,
                     eval_best_type, eval_best_value,
                     classification, cp_loss, pv, depth, clock_ms)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&review.game_id)
            .bind(ply)
            .bind(&position.fen)
            .bind(&position.played_san)
            .bind(&position.best_move_san)
            .bind(&position.best_move_uci)
            .bind(eb_type)
            .bind(eb_val)
            .bind(ea_type)
            .bind(ea_val)
            .bind(ebest_type)
            .bind(ebest_val)
            .bind(classification)
            .bind(cp_loss)
            .bind(&pv_json)
            .bind(depth)
            .bind(clock_ms)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn load_review(&self, game_id: &str) -> Result<Option<GameReview>, PersistenceError> {
        let header = sqlx::query(
            r#"
            SELECT status, status_current_ply, status_total_plies, status_error,
                   white_accuracy, black_accuracy, total_plies, analyzed_plies,
                   analysis_depth, started_at, completed_at, winner
            FROM game_reviews
            WHERE game_id = ?
            "#,
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        let row = match header {
            None => return Ok(None),
            Some(r) => r,
        };

        use sqlx::Row;
        let status_str: String = row.get("status");
        let status_current_ply: Option<i64> = row.get("status_current_ply");
        let status_total_plies: Option<i64> = row.get("status_total_plies");
        let status_error: Option<String> = row.get("status_error");
        let white_accuracy: Option<f64> = row.get("white_accuracy");
        let black_accuracy: Option<f64> = row.get("black_accuracy");
        let total_plies: i64 = row.get("total_plies");
        let analyzed_plies: i64 = row.get("analyzed_plies");
        let analysis_depth: i64 = row.get("analysis_depth");
        let started_at: Option<i64> = row.get("started_at");
        let completed_at: Option<i64> = row.get("completed_at");
        let winner: Option<String> = row.get("winner");

        let status = decode_status(
            &status_str,
            status_current_ply.map(|v| v as u32),
            status_total_plies.map(|v| v as u32),
            status_error,
        );

        let pos_rows = sqlx::query(
            r#"
            SELECT ply, fen, played_san, best_move_san, best_move_uci,
                   eval_before_type, eval_before_value,
                   eval_after_type, eval_after_value,
                   eval_best_type, eval_best_value,
                   classification, cp_loss, pv, depth, clock_ms
            FROM position_reviews
            WHERE game_id = ?
            ORDER BY ply ASC
            "#,
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        let mut positions = Vec::with_capacity(pos_rows.len());
        for pr in pos_rows {
            let ply: i64 = pr.get("ply");
            let fen: String = pr.get("fen");
            let played_san: String = pr.get("played_san");
            let best_move_san: String = pr.get("best_move_san");
            let best_move_uci: String = pr.get("best_move_uci");
            let eb_type: String = pr.get("eval_before_type");
            let eb_val: i64 = pr.get("eval_before_value");
            let ea_type: String = pr.get("eval_after_type");
            let ea_val: i64 = pr.get("eval_after_value");
            let ebest_type: String = pr.get("eval_best_type");
            let ebest_val: i64 = pr.get("eval_best_value");
            let classification_str: String = pr.get("classification");
            let cp_loss: i64 = pr.get("cp_loss");
            let pv_json: String = pr.get("pv");
            let depth: i64 = pr.get("depth");
            let clock_ms: Option<i64> = pr.get("clock_ms");

            let pv: Vec<String> = serde_json::from_str(&pv_json).unwrap_or_default();

            positions.push(PositionReview {
                ply: ply as u32,
                fen,
                played_san,
                best_move_san,
                best_move_uci,
                eval_before: decode_score(&eb_type, eb_val as i32),
                eval_after: decode_score(&ea_type, ea_val as i32),
                eval_best: decode_score(&ebest_type, ebest_val as i32),
                classification: decode_classification(&classification_str),
                cp_loss: cp_loss as i32,
                pv,
                depth: depth as u32,
                clock_ms: clock_ms.map(|v| v as u64),
            });
        }

        Ok(Some(GameReview {
            game_id: game_id.to_string(),
            status,
            positions,
            white_accuracy,
            black_accuracy,
            total_plies: total_plies as u32,
            analyzed_plies: analyzed_plies as u32,
            analysis_depth: analysis_depth as u32,
            started_at: started_at.map(|v| v as u64),
            completed_at: completed_at.map(|v| v as u64),
            winner,
        }))
    }

    async fn list_reviews(&self) -> Result<Vec<GameReview>, PersistenceError> {
        let game_ids: Vec<(String,)> = sqlx::query_as(
            "SELECT game_id FROM game_reviews ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut reviews = Vec::with_capacity(game_ids.len());
        for (game_id,) in game_ids {
            if let Some(review) = self.load_review(&game_id).await? {
                reviews.push(review);
            }
        }

        Ok(reviews)
    }

    async fn delete_review(&self, game_id: &str) -> Result<(), PersistenceError> {
        sqlx::query("DELETE FROM game_reviews WHERE game_id = ?")
            .bind(game_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use analysis::{AnalysisScore, MoveClassification, ReviewStatus};
    use crate::persistence::sqlite::Database;

    async fn test_db() -> (Database, SqliteReviewRepository) {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqliteReviewRepository::new(db.pool().clone());
        (db, repo)
    }

    /// Insert a stub finished game so FK constraints are satisfied.
    async fn insert_parent_game(db: &Database, game_id: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO finished_games \
             (game_id, start_fen, result, result_reason, game_mode, \
              human_side, skill_level, move_count, created_at) \
             VALUES (?, 'startpos', 'Draw', 'Agreement', 'HumanVsHuman', NULL, 10, 1, 0)",
        )
        .bind(game_id)
        .execute(db.pool())
        .await
        .unwrap();
    }

    fn sample_position(ply: u32) -> PositionReview {
        PositionReview {
            ply,
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            played_san: "e4".to_string(),
            best_move_san: "e4".to_string(),
            best_move_uci: "e2e4".to_string(),
            eval_before: AnalysisScore::Centipawns(20),
            eval_after: AnalysisScore::Centipawns(25),
            eval_best: AnalysisScore::Centipawns(25),
            classification: MoveClassification::Best,
            cp_loss: 0,
            pv: vec!["e5".to_string(), "Nf3".to_string()],
            depth: 18,
            clock_ms: Some(60000),
        }
    }

    fn complete_review(game_id: &str) -> GameReview {
        GameReview {
            game_id: game_id.to_string(),
            status: ReviewStatus::Complete,
            positions: vec![sample_position(1), sample_position(2)],
            white_accuracy: Some(92.5),
            black_accuracy: Some(88.3),
            total_plies: 2,
            analyzed_plies: 2,
            analysis_depth: 18,
            started_at: Some(1000),
            completed_at: Some(2000),
            winner: Some("White".to_string()),
        }
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_001").await;
        let review = complete_review("game_001");

        repo.save_review(&review).await.unwrap();
        let loaded = repo.load_review("game_001").await.unwrap().unwrap();

        assert_eq!(loaded.game_id, review.game_id);
        assert_eq!(loaded.status, review.status);
        assert_eq!(loaded.positions.len(), 2);
        assert_eq!(loaded.white_accuracy, review.white_accuracy);
        assert_eq!(loaded.black_accuracy, review.black_accuracy);
        assert_eq!(loaded.total_plies, review.total_plies);
        assert_eq!(loaded.analyzed_plies, review.analyzed_plies);
        assert_eq!(loaded.analysis_depth, review.analysis_depth);
        assert_eq!(loaded.started_at, review.started_at);
        assert_eq!(loaded.completed_at, review.completed_at);
        assert_eq!(loaded.winner, review.winner);

        let pos = &loaded.positions[0];
        assert_eq!(pos.ply, 1);
        assert_eq!(pos.played_san, "e4");
        assert!(matches!(pos.eval_before, AnalysisScore::Centipawns(20)));
        assert!(matches!(pos.classification, MoveClassification::Best));
        assert_eq!(pos.cp_loss, 0);
        assert_eq!(pos.pv, vec!["e5".to_string(), "Nf3".to_string()]);
        assert_eq!(pos.clock_ms, Some(60000));
    }

    #[tokio::test]
    async fn test_partial_review_analyzing_status() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_002").await;
        let review = GameReview {
            game_id: "game_002".to_string(),
            status: ReviewStatus::Analyzing {
                current_ply: 5,
                total_plies: 40,
            },
            positions: vec![sample_position(1), sample_position(2), sample_position(3)],
            white_accuracy: None,
            black_accuracy: None,
            total_plies: 40,
            analyzed_plies: 5,
            analysis_depth: 20,
            started_at: Some(5000),
            completed_at: None,
            winner: None,
        };

        repo.save_review(&review).await.unwrap();
        let loaded = repo.load_review("game_002").await.unwrap().unwrap();

        assert_eq!(
            loaded.status,
            ReviewStatus::Analyzing {
                current_ply: 5,
                total_plies: 40
            }
        );
        assert_eq!(loaded.positions.len(), 3);
        assert_eq!(loaded.white_accuracy, None);
        assert_eq!(loaded.completed_at, None);
        assert_eq!(loaded.winner, None);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let (_db, repo) = test_db().await;
        let result = repo.load_review("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_reviews() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_a").await;
        insert_parent_game(&db, "game_b").await;
        insert_parent_game(&db, "game_c").await;

        repo.save_review(&complete_review("game_a")).await.unwrap();
        repo.save_review(&complete_review("game_b")).await.unwrap();
        repo.save_review(&complete_review("game_c")).await.unwrap();

        let list = repo.list_reviews().await.unwrap();
        assert_eq!(list.len(), 3);
        let ids: Vec<&str> = list.iter().map(|r| r.game_id.as_str()).collect();
        assert!(ids.contains(&"game_a"));
        assert!(ids.contains(&"game_b"));
        assert!(ids.contains(&"game_c"));
    }

    #[tokio::test]
    async fn test_delete_review() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_del").await;

        repo.save_review(&complete_review("game_del")).await.unwrap();
        repo.delete_review("game_del").await.unwrap();

        let result = repo.load_review("game_del").await.unwrap();
        assert!(result.is_none());

        let list = repo.list_reviews().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_delete_cascades_positions() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_cascade").await;

        repo.save_review(&complete_review("game_cascade")).await.unwrap();
        // Verify positions were stored
        let loaded = repo.load_review("game_cascade").await.unwrap().unwrap();
        assert_eq!(loaded.positions.len(), 2);

        repo.delete_review("game_cascade").await.unwrap();
        let result = repo.load_review("game_cascade").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_insert_or_ignore_positions_on_resave() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_resave").await;

        // First save with 2 positions
        let mut review = complete_review("game_resave");
        repo.save_review(&review).await.unwrap();

        // Second save (header update) with same positions + 1 new
        review.analyzed_plies = 3;
        review.positions.push(sample_position(3));
        repo.save_review(&review).await.unwrap();

        let loaded = repo.load_review("game_resave").await.unwrap().unwrap();
        // The 2 existing positions were ignored (INSERT OR IGNORE), new one added
        assert_eq!(loaded.positions.len(), 3);
        assert_eq!(loaded.analyzed_plies, 3);
    }

    #[tokio::test]
    async fn test_failed_status_roundtrip() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_fail").await;
        let review = GameReview {
            game_id: "game_fail".to_string(),
            status: ReviewStatus::Failed {
                error: "engine timed out".to_string(),
            },
            positions: vec![],
            white_accuracy: None,
            black_accuracy: None,
            total_plies: 50,
            analyzed_plies: 3,
            analysis_depth: 18,
            started_at: Some(9000),
            completed_at: None,
            winner: None,
        };

        repo.save_review(&review).await.unwrap();
        let loaded = repo.load_review("game_fail").await.unwrap().unwrap();

        assert_eq!(
            loaded.status,
            ReviewStatus::Failed {
                error: "engine timed out".to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_mate_score_roundtrip() {
        let (db, repo) = test_db().await;
        insert_parent_game(&db, "game_mate").await;
        let mut pos = sample_position(1);
        pos.eval_before = AnalysisScore::Mate(3);
        pos.eval_after = AnalysisScore::Mate(-2);
        pos.eval_best = AnalysisScore::Mate(3);
        pos.classification = MoveClassification::Brilliant;

        let review = GameReview {
            game_id: "game_mate".to_string(),
            status: ReviewStatus::Complete,
            positions: vec![pos],
            white_accuracy: Some(100.0),
            black_accuracy: None,
            total_plies: 1,
            analyzed_plies: 1,
            analysis_depth: 20,
            started_at: None,
            completed_at: None,
            winner: Some("White".to_string()),
        };

        repo.save_review(&review).await.unwrap();
        let loaded = repo.load_review("game_mate").await.unwrap().unwrap();

        let p = &loaded.positions[0];
        assert!(matches!(p.eval_before, AnalysisScore::Mate(3)));
        assert!(matches!(p.eval_after, AnalysisScore::Mate(-2)));
        assert!(matches!(p.classification, MoveClassification::Brilliant));
    }
}
