use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Persistence layer for suspended sessions. Uses JSON files in a directory.
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    /// Create a new SessionStore with the given data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        let dir = data_dir.join("sessions");
        Self { dir }
    }

    fn new_in(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn ensure_dir(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("Failed to create sessions directory: {}", e))
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    /// Save a suspended session. Returns the suspended_id.
    pub fn save(&self, data: &SuspendedSessionData) -> Result<String, String> {
        self.ensure_dir()?;
        let path = self.file_path(&data.suspended_id);
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize session: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write session file: {}", e))?;
        Ok(data.suspended_id.clone())
    }

    /// List all suspended sessions, sorted by created_at descending (most recent first).
    pub fn list(&self) -> Result<Vec<SuspendedSessionData>, String> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut sessions = Vec::new();
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| format!("Failed to read sessions directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        if let Ok(data) = serde_json::from_str::<SuspendedSessionData>(&contents) {
                            sessions.push(data);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read session file {:?}: {}", path, e);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    /// Load a specific suspended session by ID.
    pub fn load(&self, id: &str) -> Result<Option<SuspendedSessionData>, String> {
        let path = self.file_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read session file: {}", e))?;
        let data = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse session file: {}", e))?;
        Ok(Some(data))
    }

    /// Delete a suspended session by ID.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.file_path(id);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete session file: {}", e))?;
        }
        Ok(())
    }

    /// Check if any suspended sessions exist.
    pub fn has_any(&self) -> bool {
        self.list().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

/// Generate a unique suspended session ID using timestamp + random suffix.
pub fn generate_suspended_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("session_{}", ts)
}

/// Get the current unix timestamp in seconds.
pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============================================================================
// Position Storage
// ============================================================================

/// Data stored for a saved position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedPositionData {
    pub position_id: String,
    pub name: String,
    pub fen: String,
    pub is_default: bool,
    pub created_at: u64,
}

/// Persistence layer for saved positions. Uses JSON files in a directory.
pub struct PositionStore {
    dir: PathBuf,
    defaults_dir: Option<PathBuf>,
}

impl PositionStore {
    /// Create a new PositionStore with runtime data directory and optional defaults directory.
    ///
    /// If defaults_dir is provided, default positions will be copied from there on initialization.
    pub fn new(data_dir: PathBuf, defaults_dir: Option<PathBuf>) -> Self {
        let dir = data_dir.join("positions");
        let store = Self { dir, defaults_dir };
        store.seed_defaults();
        store
    }

    fn new_in(dir: PathBuf) -> Self {
        Self {
            dir,
            defaults_dir: None,
        }
    }

    fn ensure_dir(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("Failed to create positions directory: {}", e))
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    /// Seed default positions if none exist.
    ///
    /// If a defaults_dir is configured, copies default position files from there.
    /// Otherwise, creates a minimal set of hardcoded defaults as fallback.
    fn seed_defaults(&self) {
        if let Ok(existing) = self.list() {
            if existing.iter().any(|p| p.is_default) {
                return; // Already seeded
            }
        }

        // Try to copy from defaults directory first
        if let Some(ref defaults_dir) = self.defaults_dir {
            let defaults_positions_dir = defaults_dir.join("positions");
            if defaults_positions_dir.exists() {
                if let Err(e) = self.copy_defaults_from(&defaults_positions_dir) {
                    tracing::warn!(
                        "Failed to copy defaults from {:?}: {}",
                        defaults_positions_dir,
                        e
                    );
                    self.create_fallback_defaults();
                }
                return;
            }
        }

        // Fallback: create minimal hardcoded defaults
        self.create_fallback_defaults();
    }

    /// Copy default position files from a directory.
    fn copy_defaults_from(&self, source_dir: &PathBuf) -> Result<(), String> {
        self.ensure_dir()?;

        let entries = std::fs::read_dir(source_dir)
            .map_err(|e| format!("Failed to read defaults directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(filename) = path.file_name() {
                    let dest = self.dir.join(filename);
                    std::fs::copy(&path, &dest)
                        .map_err(|e| format!("Failed to copy {:?}: {}", filename, e))?;
                }
            }
        }

        tracing::info!("Copied default positions from {:?}", source_dir);
        Ok(())
    }

    /// Create minimal hardcoded defaults as fallback.
    fn create_fallback_defaults(&self) {
        let defaults = vec![(
            "Standard Starting Position",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        )];

        for (name, fen) in defaults {
            let id = format!("default_{}", name.to_lowercase().replace(' ', "_"));
            let data = SavedPositionData {
                position_id: id,
                name: name.to_string(),
                fen: fen.to_string(),
                is_default: true,
                created_at: 0,
            };
            let _ = self.save(&data);
        }
        tracing::info!("Created fallback default positions");
    }

    /// Save a position. Returns the position_id.
    pub fn save(&self, data: &SavedPositionData) -> Result<String, String> {
        self.ensure_dir()?;
        let path = self.file_path(&data.position_id);
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize position: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write position file: {}", e))?;
        Ok(data.position_id.clone())
    }

    /// List all positions, defaults first then user positions sorted by created_at.
    pub fn list(&self) -> Result<Vec<SavedPositionData>, String> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut positions = Vec::new();
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| format!("Failed to read positions directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<SavedPositionData>(&contents) {
                        positions.push(data);
                    }
                }
            }
        }

        // Sort: defaults first (by name), then user positions by created_at descending
        positions.sort_by(|a, b| match (a.is_default, b.is_default) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => a.name.cmp(&b.name),
            (false, false) => b.created_at.cmp(&a.created_at),
        });

        Ok(positions)
    }

    /// Delete a position by ID. Returns error if it's a default position.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.file_path(id);
        if path.exists() {
            // Check if it's a default
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<SavedPositionData>(&contents) {
                    if data.is_default {
                        return Err("Cannot delete default positions".to_string());
                    }
                }
            }
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete position file: {}", e))?;
        }
        Ok(())
    }
}

/// Generate a unique position ID.
pub fn generate_position_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("pos_{}", ts)
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
        store.ensure_dir().unwrap();
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

    // Position store tests

    fn sample_position(id: &str, name: &str, is_default: bool) -> SavedPositionData {
        SavedPositionData {
            position_id: id.to_string(),
            name: name.to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            is_default,
            created_at: 100,
        }
    }

    #[test]
    fn test_position_save_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let store = PositionStore::new_in(dir.path().join("positions"));
        store
            .save(&sample_position("p1", "My Opening", false))
            .unwrap();
        store.save(&sample_position("p2", "Default", true)).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
        // Defaults come first
        assert!(list[0].is_default);
    }

    #[test]
    fn test_position_delete_user() {
        let dir = tempfile::tempdir().unwrap();
        let store = PositionStore::new_in(dir.path().join("positions"));
        store
            .save(&sample_position("user1", "My Pos", false))
            .unwrap();
        store.delete("user1").unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn test_position_cannot_delete_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = PositionStore::new_in(dir.path().join("positions"));
        store
            .save(&sample_position("def1", "Default Pos", true))
            .unwrap();
        let result = store.delete("def1");
        assert!(result.is_err());
    }
}
