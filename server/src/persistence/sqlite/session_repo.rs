//! SQLite-backed repository for suspended sessions.

use sqlx::SqlitePool;

use crate::persistence::{PersistenceError, SuspendedSessionData};
use crate::persistence::traits::SessionRepository;
use super::helpers::normalize_game_mode;

/// SQLite implementation of [`SessionRepository`].
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl SessionRepository for SqliteSessionRepository {
    async fn save_session(&self, data: &SuspendedSessionData) -> Result<(), PersistenceError> {
        let game_mode = normalize_game_mode(&data.game_mode);
        let move_count = data.move_count as i64;
        let skill_level = data.skill_level as i64;
        let created_at = data.created_at as i64;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO suspended_sessions
                (suspended_id, fen, side_to_move, move_count, game_mode,
                 human_side, skill_level, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&data.suspended_id)
        .bind(&data.fen)
        .bind(&data.side_to_move)
        .bind(move_count)
        .bind(game_mode)
        .bind(&data.human_side)
        .bind(skill_level)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<SuspendedSessionData>, PersistenceError> {
        let rows: Vec<(String, String, String, i64, String, Option<String>, i64, i64)> =
            sqlx::query_as(
                r#"
                SELECT suspended_id, fen, side_to_move, move_count, game_mode,
                       human_side, skill_level, created_at
                FROM suspended_sessions
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?;

        let sessions = rows
            .into_iter()
            .map(
                |(suspended_id, fen, side_to_move, move_count, game_mode, human_side, skill_level, created_at)| {
                    SuspendedSessionData {
                        suspended_id,
                        fen,
                        side_to_move,
                        move_count: move_count as u32,
                        game_mode,
                        human_side,
                        skill_level: skill_level as u8,
                        created_at: created_at as u64,
                    }
                },
            )
            .collect();

        Ok(sessions)
    }

    async fn load_session(&self, id: &str) -> Result<Option<SuspendedSessionData>, PersistenceError> {
        let row: Option<(String, String, String, i64, String, Option<String>, i64, i64)> =
            sqlx::query_as(
                r#"
                SELECT suspended_id, fen, side_to_move, move_count, game_mode,
                       human_side, skill_level, created_at
                FROM suspended_sessions
                WHERE suspended_id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(
            |(suspended_id, fen, side_to_move, move_count, game_mode, human_side, skill_level, created_at)| {
                SuspendedSessionData {
                    suspended_id,
                    fen,
                    side_to_move,
                    move_count: move_count as u32,
                    game_mode,
                    human_side,
                    skill_level: skill_level as u8,
                    created_at: created_at as u64,
                }
            },
        ))
    }

    async fn delete_session(&self, id: &str) -> Result<(), PersistenceError> {
        sqlx::query("DELETE FROM suspended_sessions WHERE suspended_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::Database;

    async fn test_db() -> (Database, SqliteSessionRepository) {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqliteSessionRepository::new(db.pool().clone());
        (db, repo)
    }

    fn sample_session(id: &str, ts: u64) -> SuspendedSessionData {
        SuspendedSessionData {
            suspended_id: id.to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            side_to_move: "black".to_string(),
            move_count: 1,
            game_mode: "HumanVsEngine".to_string(),
            human_side: Some("white".to_string()),
            skill_level: 10,
            created_at: ts,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let (_db, repo) = test_db().await;
        let data = sample_session("sess_1", 1000);
        repo.save_session(&data).await.unwrap();
        let loaded = repo.load_session("sess_1").await.unwrap();
        assert_eq!(loaded, Some(data));
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let (_db, repo) = test_db().await;
        let loaded = repo.load_session("nonexistent").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_list_ordering() {
        let (_db, repo) = test_db().await;
        repo.save_session(&sample_session("old", 100)).await.unwrap();
        repo.save_session(&sample_session("mid", 200)).await.unwrap();
        repo.save_session(&sample_session("new", 300)).await.unwrap();

        let list = repo.list_sessions().await.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].suspended_id, "new");
        assert_eq!(list[1].suspended_id, "mid");
        assert_eq!(list[2].suspended_id, "old");
    }

    #[tokio::test]
    async fn test_delete_session() {
        let (_db, repo) = test_db().await;
        repo.save_session(&sample_session("to_delete", 100)).await.unwrap();
        repo.delete_session("to_delete").await.unwrap();
        let loaded = repo.load_session("to_delete").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_save_normalizes_game_mode() {
        let (_db, repo) = test_db().await;
        let mut data = sample_session("sess_norm", 500);
        data.game_mode = "HumanVsEngine:White".to_string();
        // normalize_game_mode strips the ":White" suffix before INSERT
        repo.save_session(&data).await.unwrap();
        let loaded = repo.load_session("sess_norm").await.unwrap().unwrap();
        assert_eq!(loaded.game_mode, "HumanVsEngine");
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (_db, repo) = test_db().await;
        let list = repo.list_sessions().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_save_replace() {
        let (_db, repo) = test_db().await;
        let mut data = sample_session("sess_replace", 100);
        repo.save_session(&data).await.unwrap();
        data.skill_level = 20;
        repo.save_session(&data).await.unwrap();
        let loaded = repo.load_session("sess_replace").await.unwrap().unwrap();
        assert_eq!(loaded.skill_level, 20);
        let list = repo.list_sessions().await.unwrap();
        assert_eq!(list.len(), 1);
    }
}
