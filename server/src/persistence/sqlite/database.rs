//! SQLite database connection pool and migration runner.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

use crate::persistence::PersistenceError;

/// Holds a connection pool to the SQLite database.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (or create) the database at `path`, run migrations, and return
    /// a ready-to-use `Database`.
    pub async fn open(path: &Path) -> Result<Self, PersistenceError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(PersistenceError::Io)?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .map_err(sqlx::Error::from)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(sqlx::Error::from)?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create an in-memory database for testing. Migrations are applied.
    #[cfg(test)]
    pub async fn new_in_memory() -> Result<Self, PersistenceError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(sqlx::Error::from)?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(sqlx::Error::from)?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Run embedded migrations from `server/migrations/`.
    async fn run_migrations(&self) -> Result<(), PersistenceError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| PersistenceError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Get a reference to the underlying pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_in_memory() {
        let db = Database::new_in_memory().await.unwrap();
        // Verify the pool is functional
        let row: (i64,) = sqlx::query_as("SELECT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(row.0, 1);
    }

    #[tokio::test]
    async fn test_migrations_create_tables() {
        let db = Database::new_in_memory().await.unwrap();
        // Verify key tables exist
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        assert!(names.contains(&"suspended_sessions"));
        assert!(names.contains(&"saved_positions"));
        assert!(names.contains(&"finished_games"));
        assert!(names.contains(&"stored_moves"));
        assert!(names.contains(&"game_reviews"));
        assert!(names.contains(&"position_reviews"));
    }

    #[tokio::test]
    async fn test_open_file_based() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).await.unwrap();
        let row: (i64,) = sqlx::query_as("SELECT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(row.0, 1);
        assert!(db_path.exists());
    }
}
