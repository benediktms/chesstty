use super::json_store::{JsonStore, Storable};
use super::PersistenceError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Data stored for a suspended session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuspendedSessionData {
    pub suspended_id: String,
    pub fen: String,
    pub side_to_move: String,
    pub move_count: u32,
    pub game_mode: String,
    pub human_side: Option<String>,
    pub skill_level: u8,
    pub created_at: u64,
}

impl Storable for SuspendedSessionData {
    fn id(&self) -> &str {
        &self.suspended_id
    }
}

/// Persistence layer for suspended sessions. Uses JSON files in a directory.
/// Kept as a fallback trait implementation; production uses SqliteSessionRepository.
#[allow(dead_code)]
pub struct SessionStore {
    inner: JsonStore<SuspendedSessionData>,
}

#[allow(dead_code)]
impl SessionStore {
    /// Create a new SessionStore with the given data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        let dir = data_dir.join("sessions");
        Self {
            inner: JsonStore::new(dir),
        }
    }

    /// Save a suspended session. Returns the suspended_id.
    pub fn save(&self, data: &SuspendedSessionData) -> Result<String, PersistenceError> {
        self.inner.save(data)
    }

    /// List all suspended sessions, sorted by created_at descending (most recent first).
    pub fn list(&self) -> Result<Vec<SuspendedSessionData>, PersistenceError> {
        let mut sessions = self.inner.load_all()?;
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    /// Load a specific suspended session by ID.
    pub fn load(&self, id: &str) -> Result<Option<SuspendedSessionData>, PersistenceError> {
        self.inner.load(id)
    }

    /// Delete a suspended session by ID.
    pub fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        self.inner.delete(id)
    }
}

impl super::traits::SessionRepository for SessionStore {
    async fn save_session(&self, data: &SuspendedSessionData) -> Result<(), super::PersistenceError> {
        self.save(data)?;
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<SuspendedSessionData>, super::PersistenceError> {
        self.list()
    }

    async fn load_session(&self, id: &str) -> Result<Option<SuspendedSessionData>, super::PersistenceError> {
        self.load(id)
    }

    async fn delete_session(&self, id: &str) -> Result<(), super::PersistenceError> {
        self.delete(id)
    }
}

#[cfg(test)]
impl SessionStore {
    fn new_in(dir: PathBuf) -> Self {
        Self {
            inner: JsonStore::new(dir),
        }
    }

    fn has_any(&self) -> bool {
        self.list().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data(id: &str, ts: u64) -> SuspendedSessionData {
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

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        let data = sample_data("test_1", 100);
        store.save(&data).unwrap();
        let loaded = store.load("test_1").unwrap();
        assert_eq!(loaded, Some(data));
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        store.inner.ensure_dir().unwrap();
        let loaded = store.load("nonexistent").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_list_multiple_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        store.save(&sample_data("old", 100)).unwrap();
        store.save(&sample_data("mid", 200)).unwrap();
        store.save(&sample_data("new", 300)).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].suspended_id, "new");
        assert_eq!(list[1].suspended_id, "mid");
        assert_eq!(list[2].suspended_id, "old");
    }

    #[test]
    fn test_delete_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        store.save(&sample_data("to_delete", 100)).unwrap();
        store.delete("to_delete").unwrap();
        assert_eq!(store.load("to_delete").unwrap(), None);
    }

    #[test]
    fn test_has_any() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        assert!(!store.has_any());
        store.save(&sample_data("one", 100)).unwrap();
        assert!(store.has_any());
    }

    #[test]
    fn test_list_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new_in(dir.path().join("sessions"));
        let list = store.list().unwrap();
        assert!(list.is_empty());
    }
}
