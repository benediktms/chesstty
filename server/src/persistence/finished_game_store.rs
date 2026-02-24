use super::json_store::{JsonStore, Storable};
use super::PersistenceError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A move record stored in a finished game, serializable to JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredMoveRecord {
    pub from: String,
    pub to: String,
    pub piece: String,
    pub captured: Option<String>,
    pub promotion: Option<String>,
    pub san: String,
    pub fen_after: String,
    #[serde(default)]
    pub clock_ms: Option<u64>,
}

/// Data stored for a completed game eligible for review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinishedGameData {
    pub game_id: String,
    pub start_fen: String,
    pub result: String,
    pub result_reason: String,
    pub game_mode: String,
    pub human_side: Option<String>,
    pub skill_level: u8,
    pub move_count: u32,
    pub moves: Vec<StoredMoveRecord>,
    pub created_at: u64,
}

impl Storable for FinishedGameData {
    fn id(&self) -> &str {
        &self.game_id
    }
}

/// Persistence layer for finished games. Uses JSON files in a directory.
/// Kept as a fallback trait implementation; production uses SqliteFinishedGameRepository.
#[allow(dead_code)]
pub struct FinishedGameStore {
    inner: JsonStore<FinishedGameData>,
}

#[allow(dead_code)]
impl FinishedGameStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let dir = data_dir.join("finished_games");
        Self {
            inner: JsonStore::new(dir),
        }
    }

    /// Save a finished game. Returns the game_id.
    pub fn save(&self, data: &FinishedGameData) -> Result<String, PersistenceError> {
        self.inner.save(data)
    }

    /// List all finished games, sorted by created_at descending (most recent first).
    pub fn list(&self) -> Result<Vec<FinishedGameData>, PersistenceError> {
        let mut games = self.inner.load_all()?;
        games.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(games)
    }

    /// Load a specific finished game by ID.
    pub fn load(&self, id: &str) -> Result<Option<FinishedGameData>, PersistenceError> {
        self.inner.load(id)
    }

    /// Delete a finished game by ID.
    pub fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        self.inner.delete(id)
    }
}

impl super::traits::FinishedGameRepository for FinishedGameStore {
    async fn save_game(&self, data: &FinishedGameData) -> Result<(), super::PersistenceError> {
        self.save(data)?;
        Ok(())
    }

    async fn list_games(&self) -> Result<Vec<FinishedGameData>, super::PersistenceError> {
        self.list()
    }

    async fn load_game(
        &self,
        id: &str,
    ) -> Result<Option<FinishedGameData>, super::PersistenceError> {
        self.load(id)
    }

    async fn delete_game(&self, id: &str) -> Result<(), super::PersistenceError> {
        self.delete(id)
    }
}

#[cfg(test)]
impl FinishedGameStore {
    fn new_in(dir: PathBuf) -> Self {
        Self {
            inner: JsonStore::new(dir),
        }
    }

    fn ensure_dir(&self) -> Result<(), PersistenceError> {
        self.inner.ensure_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_finished_game(id: &str, ts: u64) -> FinishedGameData {
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
                    clock_ms: None,
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

    #[test]
    fn test_finished_game_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FinishedGameStore::new_in(dir.path().join("finished_games"));
        let data = sample_finished_game("game_1", 100);
        store.save(&data).unwrap();
        let loaded = store.load("game_1").unwrap();
        assert_eq!(loaded, Some(data));
    }

    #[test]
    fn test_finished_game_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = FinishedGameStore::new_in(dir.path().join("finished_games"));
        store.ensure_dir().unwrap();
        let loaded = store.load("nonexistent").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_finished_game_list_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FinishedGameStore::new_in(dir.path().join("finished_games"));
        store.save(&sample_finished_game("old", 100)).unwrap();
        store.save(&sample_finished_game("mid", 200)).unwrap();
        store.save(&sample_finished_game("new", 300)).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].game_id, "new");
        assert_eq!(list[1].game_id, "mid");
        assert_eq!(list[2].game_id, "old");
    }

    #[test]
    fn test_finished_game_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = FinishedGameStore::new_in(dir.path().join("finished_games"));
        store.save(&sample_finished_game("to_delete", 100)).unwrap();
        store.delete("to_delete").unwrap();
        assert_eq!(store.load("to_delete").unwrap(), None);
    }

    #[test]
    fn test_finished_game_list_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = FinishedGameStore::new_in(dir.path().join("finished_games"));
        let list = store.list().unwrap();
        assert!(list.is_empty());
    }
}
