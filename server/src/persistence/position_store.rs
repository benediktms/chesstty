use super::json_store::{JsonStore, Storable};
use super::PersistenceError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Data stored for a saved position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedPositionData {
    pub position_id: String,
    pub name: String,
    pub fen: String,
    pub is_default: bool,
    pub created_at: u64,
}

impl Storable for SavedPositionData {
    fn id(&self) -> &str {
        &self.position_id
    }
}

/// Persistence layer for saved positions. Uses JSON files in a directory.
pub struct PositionStore {
    inner: JsonStore<SavedPositionData>,
    defaults_dir: Option<PathBuf>,
}

impl PositionStore {
    /// Create a new PositionStore with runtime data directory and optional defaults directory.
    ///
    /// If defaults_dir is provided, default positions will be copied from there on initialization.
    pub fn new(data_dir: PathBuf, defaults_dir: Option<PathBuf>) -> Self {
        let dir = data_dir.join("positions");
        let store = Self {
            inner: JsonStore::new(dir),
            defaults_dir,
        };
        store.seed_defaults();
        store
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
    fn copy_defaults_from(&self, source_dir: &PathBuf) -> Result<(), PersistenceError> {
        self.inner.ensure_dir()?;

        let entries = std::fs::read_dir(source_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(filename) = path.file_name() {
                    let dest = self.inner.dir().join(filename);
                    std::fs::copy(&path, &dest)?;
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
    pub fn save(&self, data: &SavedPositionData) -> Result<String, PersistenceError> {
        self.inner.save(data)
    }

    /// List all positions, defaults first then user positions sorted by created_at.
    pub fn list(&self) -> Result<Vec<SavedPositionData>, PersistenceError> {
        let mut positions = self.inner.load_all()?;

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
    pub fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        if let Some(data) = self.inner.load(id)? {
            if data.is_default {
                return Err(PersistenceError::DefaultPositionProtected);
            }
        }
        self.inner.delete(id)
    }
}

#[cfg(test)]
impl PositionStore {
    fn new_in(dir: PathBuf) -> Self {
        Self {
            inner: JsonStore::new(dir),
            defaults_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
