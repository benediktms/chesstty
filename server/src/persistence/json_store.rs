use super::PersistenceError;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;

/// Trait for types that can be persisted in a JsonStore.
pub trait Storable: Serialize + DeserializeOwned {
    fn id(&self) -> &str;
}

/// Generic JSON-file-per-record persistence store.
pub struct JsonStore<T> {
    dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T: Storable> JsonStore<T> {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            _phantom: PhantomData,
        }
    }

    pub fn ensure_dir(&self) -> Result<(), PersistenceError> {
        std::fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    pub fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    /// Save a record. Returns the id.
    pub fn save(&self, data: &T) -> Result<String, PersistenceError> {
        self.ensure_dir()?;
        let path = self.file_path(data.id());
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&path, json)?;
        Ok(data.id().to_string())
    }

    /// Load a record by id. Returns None if not found.
    pub fn load(&self, id: &str) -> Result<Option<T>, PersistenceError> {
        let path = self.file_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&path)?;
        let data = serde_json::from_str(&contents)?;
        Ok(Some(data))
    }

    /// Load all records from the store directory, skipping files that fail to parse.
    pub fn load_all(&self) -> Result<Vec<T>, PersistenceError> {
        if !self.dir.exists() {
            return Ok(vec![]);
        }
        let mut items = Vec::new();
        let entries = std::fs::read_dir(&self.dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        if let Ok(data) = serde_json::from_str::<T>(&contents) {
                            items.push(data);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read file {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(items)
    }

    /// Delete a record by id.
    pub fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        let path = self.file_path(id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
