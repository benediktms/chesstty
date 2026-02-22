//! SQLite-backed repository for finished games.

use sqlx::SqlitePool;

use crate::persistence::{FinishedGameData, PersistenceError, StoredMoveRecord};
use crate::persistence::traits::FinishedGameRepository;
use super::helpers::normalize_game_mode;

/// SQLite implementation of [`FinishedGameRepository`].
pub struct SqliteFinishedGameRepository {
    pool: SqlitePool,
}

impl SqliteFinishedGameRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl FinishedGameRepository for SqliteFinishedGameRepository {
    async fn save_game(&self, data: &FinishedGameData) -> Result<(), PersistenceError> {
        let game_mode = normalize_game_mode(&data.game_mode);
        let skill_level = data.skill_level as i64;
        let move_count = data.move_count as i64;
        let created_at = data.created_at as i64;

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO finished_games
                (game_id, start_fen, result, result_reason, game_mode,
                 human_side, skill_level, move_count, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&data.game_id)
        .bind(&data.start_fen)
        .bind(&data.result)
        .bind(&data.result_reason)
        .bind(game_mode)
        .bind(&data.human_side)
        .bind(skill_level)
        .bind(move_count)
        .bind(created_at)
        .execute(&mut *tx)
        .await?;

        // Delete existing moves for this game before re-inserting
        sqlx::query("DELETE FROM stored_moves WHERE game_id = ?")
            .bind(&data.game_id)
            .execute(&mut *tx)
            .await?;

        for (ply, mv) in data.moves.iter().enumerate() {
            let ply = ply as i64;
            let clock_ms = mv.clock_ms.map(|v| v as i64);
            sqlx::query(
                r#"
                INSERT INTO stored_moves
                    (game_id, ply, mv_from, mv_to, piece, captured,
                     promotion, san, fen_after, clock_ms)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&data.game_id)
            .bind(ply)
            .bind(&mv.from)
            .bind(&mv.to)
            .bind(&mv.piece)
            .bind(&mv.captured)
            .bind(&mv.promotion)
            .bind(&mv.san)
            .bind(&mv.fen_after)
            .bind(clock_ms)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_games(&self) -> Result<Vec<FinishedGameData>, PersistenceError> {
        let game_rows: Vec<(String, String, String, String, String, Option<String>, i64, i64, i64)> =
            sqlx::query_as(
                r#"
                SELECT game_id, start_fen, result, result_reason, game_mode,
                       human_side, skill_level, move_count, created_at
                FROM finished_games
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?;

        let mut games = Vec::with_capacity(game_rows.len());
        for (game_id, start_fen, result, result_reason, game_mode, human_side, skill_level, move_count, created_at) in game_rows {
            let moves = load_moves_for_game(&self.pool, &game_id).await?;
            games.push(FinishedGameData {
                game_id,
                start_fen,
                result,
                result_reason,
                game_mode,
                human_side,
                skill_level: skill_level as u8,
                move_count: move_count as u32,
                moves,
                created_at: created_at as u64,
            });
        }

        Ok(games)
    }

    async fn load_game(&self, id: &str) -> Result<Option<FinishedGameData>, PersistenceError> {
        let row: Option<(String, String, String, String, String, Option<String>, i64, i64, i64)> =
            sqlx::query_as(
                r#"
                SELECT game_id, start_fen, result, result_reason, game_mode,
                       human_side, skill_level, move_count, created_at
                FROM finished_games
                WHERE game_id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Ok(None),
            Some((game_id, start_fen, result, result_reason, game_mode, human_side, skill_level, move_count, created_at)) => {
                let moves = load_moves_for_game(&self.pool, &game_id).await?;
                Ok(Some(FinishedGameData {
                    game_id,
                    start_fen,
                    result,
                    result_reason,
                    game_mode,
                    human_side,
                    skill_level: skill_level as u8,
                    move_count: move_count as u32,
                    moves,
                    created_at: created_at as u64,
                }))
            }
        }
    }

    async fn delete_game(&self, id: &str) -> Result<(), PersistenceError> {
        sqlx::query("DELETE FROM finished_games WHERE game_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Load all moves for a game ordered by ply.
async fn load_moves_for_game(
    pool: &SqlitePool,
    game_id: &str,
) -> Result<Vec<StoredMoveRecord>, PersistenceError> {
    let rows: Vec<(String, String, String, Option<String>, Option<String>, String, String, Option<i64>)> =
        sqlx::query_as(
            r#"
            SELECT mv_from, mv_to, piece, captured, promotion, san, fen_after, clock_ms
            FROM stored_moves
            WHERE game_id = ?
            ORDER BY ply
            "#,
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(from, to, piece, captured, promotion, san, fen_after, clock_ms)| StoredMoveRecord {
            from,
            to,
            piece,
            captured,
            promotion,
            san,
            fen_after,
            clock_ms: clock_ms.map(|v| v as u64),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::Database;

    async fn test_db() -> (Database, SqliteFinishedGameRepository) {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqliteFinishedGameRepository::new(db.pool().clone());
        (db, repo)
    }

    fn sample_game(id: &str, ts: u64) -> FinishedGameData {
        FinishedGameData {
            game_id: id.to_string(),
            start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            result: "WhiteWins".to_string(),
            result_reason: "Checkmate".to_string(),
            game_mode: "HumanVsEngine".to_string(),
            human_side: Some("white".to_string()),
            skill_level: 10,
            move_count: 2,
            moves: vec![
                StoredMoveRecord {
                    from: "e2".to_string(),
                    to: "e4".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    promotion: None,
                    san: "e4".to_string(),
                    fen_after: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
                        .to_string(),
                    clock_ms: Some(5000),
                },
                StoredMoveRecord {
                    from: "e7".to_string(),
                    to: "e5".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    promotion: None,
                    san: "e5".to_string(),
                    fen_after: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
                        .to_string(),
                    clock_ms: None,
                },
            ],
            created_at: ts,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let (_db, repo) = test_db().await;
        let data = sample_game("game_1", 1000);
        repo.save_game(&data).await.unwrap();
        let loaded = repo.load_game("game_1").await.unwrap();
        assert_eq!(loaded, Some(data));
    }

    #[tokio::test]
    async fn test_moves_preserved_in_roundtrip() {
        let (_db, repo) = test_db().await;
        let data = sample_game("game_moves", 1000);
        repo.save_game(&data).await.unwrap();
        let loaded = repo.load_game("game_moves").await.unwrap().unwrap();
        assert_eq!(loaded.moves.len(), 2);
        assert_eq!(loaded.moves[0].from, "e2");
        assert_eq!(loaded.moves[0].clock_ms, Some(5000));
        assert_eq!(loaded.moves[1].from, "e7");
        assert_eq!(loaded.moves[1].clock_ms, None);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let (_db, repo) = test_db().await;
        let loaded = repo.load_game("nonexistent").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_list_ordering() {
        let (_db, repo) = test_db().await;
        repo.save_game(&sample_game("old", 100)).await.unwrap();
        repo.save_game(&sample_game("mid", 200)).await.unwrap();
        repo.save_game(&sample_game("new", 300)).await.unwrap();

        let list = repo.list_games().await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].game_id, "new");
        assert_eq!(list[1].game_id, "mid");
        assert_eq!(list[2].game_id, "old");
    }

    #[tokio::test]
    async fn test_delete_cascades_moves() {
        let (_db, repo) = test_db().await;
        repo.save_game(&sample_game("to_delete", 100)).await.unwrap();
        // Verify moves exist
        let loaded = repo.load_game("to_delete").await.unwrap().unwrap();
        assert_eq!(loaded.moves.len(), 2);

        repo.delete_game("to_delete").await.unwrap();

        // Game and its moves should be gone
        let after = repo.load_game("to_delete").await.unwrap();
        assert_eq!(after, None);

        // Verify moves were cascaded - try via pool directly
        let pool = repo.pool.clone();
        let move_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM stored_moves WHERE game_id = 'to_delete'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(move_count.0, 0);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (_db, repo) = test_db().await;
        let list = repo.list_games().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_save_normalizes_game_mode() {
        let (_db, repo) = test_db().await;
        let mut data = sample_game("game_norm", 500);
        data.game_mode = "HumanVsEngine:White".to_string();
        repo.save_game(&data).await.unwrap();
        let loaded = repo.load_game("game_norm").await.unwrap().unwrap();
        assert_eq!(loaded.game_mode, "HumanVsEngine");
    }

    #[tokio::test]
    async fn test_save_replace_updates_moves() {
        let (_db, repo) = test_db().await;
        let mut data = sample_game("game_replace", 100);
        repo.save_game(&data).await.unwrap();

        // Update the game with a different move list
        data.moves = vec![StoredMoveRecord {
            from: "d2".to_string(),
            to: "d4".to_string(),
            piece: "P".to_string(),
            captured: None,
            promotion: None,
            san: "d4".to_string(),
            fen_after: "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1".to_string(),
            clock_ms: None,
        }];
        data.move_count = 1;
        repo.save_game(&data).await.unwrap();

        let loaded = repo.load_game("game_replace").await.unwrap().unwrap();
        assert_eq!(loaded.moves.len(), 1);
        assert_eq!(loaded.moves[0].from, "d2");
    }
}
