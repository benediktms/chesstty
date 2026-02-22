//! SQLite-backed implementation of [`PositionRepository`].

use sqlx::SqlitePool;

use crate::persistence::{PersistenceError, SavedPositionData};
use crate::persistence::traits::PositionRepository;

pub struct SqlitePositionRepository {
    pool: SqlitePool,
}

impl SqlitePositionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl PositionRepository for SqlitePositionRepository {
    async fn save_position(&self, data: &SavedPositionData) -> Result<(), PersistenceError> {
        let is_default: i64 = if data.is_default { 1 } else { 0 };
        let created_at = data.created_at as i64;

        sqlx::query(
            "INSERT OR REPLACE INTO saved_positions \
             (position_id, name, fen, is_default, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&data.position_id)
        .bind(&data.name)
        .bind(&data.fen)
        .bind(is_default)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_positions(&self) -> Result<Vec<SavedPositionData>, PersistenceError> {
        let rows: Vec<(String, String, String, i64, i64)> = sqlx::query_as(
            "SELECT position_id, name, fen, is_default, created_at \
             FROM saved_positions \
             ORDER BY is_default DESC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let positions = rows
            .into_iter()
            .map(|(position_id, name, fen, is_default, created_at)| SavedPositionData {
                position_id,
                name,
                fen,
                is_default: is_default != 0,
                created_at: created_at as u64,
            })
            .collect();

        Ok(positions)
    }

    async fn delete_position(&self, id: &str) -> Result<(), PersistenceError> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT is_default FROM saved_positions WHERE position_id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        if let Some((is_default,)) = row {
            if is_default != 0 {
                return Err(PersistenceError::DefaultPositionProtected);
            }
        }

        sqlx::query("DELETE FROM saved_positions WHERE position_id = ?")
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

    fn make_position(id: &str, name: &str, is_default: bool, created_at: u64) -> SavedPositionData {
        SavedPositionData {
            position_id: id.to_string(),
            name: name.to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            is_default,
            created_at,
        }
    }

    #[tokio::test]
    async fn test_save_and_list_roundtrip() {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqlitePositionRepository::new(db.pool().clone());

        let pos = make_position("p1", "My Opening", false, 1000);
        repo.save_position(&pos).await.unwrap();

        let list = repo.list_positions().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].position_id, "p1");
        assert_eq!(list[0].name, "My Opening");
        assert!(!list[0].is_default);
        assert_eq!(list[0].created_at, 1000);
    }

    #[tokio::test]
    async fn test_list_ordering_defaults_first() {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqlitePositionRepository::new(db.pool().clone());

        repo.save_position(&make_position("user1", "User Pos", false, 2000))
            .await
            .unwrap();
        repo.save_position(&make_position("def1", "Default Pos", true, 1000))
            .await
            .unwrap();
        repo.save_position(&make_position("user2", "Another User", false, 3000))
            .await
            .unwrap();

        let list = repo.list_positions().await.unwrap();
        assert_eq!(list.len(), 3);
        // Default must come first
        assert!(list[0].is_default, "first entry should be default");
        assert_eq!(list[0].position_id, "def1");
        // Remaining user positions ordered by created_at DESC
        assert_eq!(list[1].position_id, "user2");
        assert_eq!(list[2].position_id, "user1");
    }

    #[tokio::test]
    async fn test_delete_user_position() {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqlitePositionRepository::new(db.pool().clone());

        repo.save_position(&make_position("user1", "My Pos", false, 500))
            .await
            .unwrap();

        repo.delete_position("user1").await.unwrap();

        let list = repo.list_positions().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_cannot_delete_default_position() {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqlitePositionRepository::new(db.pool().clone());

        repo.save_position(&make_position("def1", "Starting Position", true, 0))
            .await
            .unwrap();

        let result = repo.delete_position("def1").await;
        assert!(
            matches!(result, Err(PersistenceError::DefaultPositionProtected)),
            "expected DefaultPositionProtected, got {:?}",
            result
        );

        // Confirm it still exists
        let list = repo.list_positions().await.unwrap();
        assert_eq!(list.len(), 1);
    }
}
