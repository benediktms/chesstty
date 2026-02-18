use analysis::AdvancedGameAnalysis;
use crate::persistence::{JsonStore, PersistenceError, Storable};
use std::path::PathBuf;

impl Storable for AdvancedGameAnalysis {
    fn id(&self) -> &str {
        &self.game_id
    }
}

/// Persistence layer for advanced game analyses.
pub struct AdvancedAnalysisStore {
    inner: JsonStore<AdvancedGameAnalysis>,
}

impl AdvancedAnalysisStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let dir = data_dir.join("advanced_reviews");
        Self {
            inner: JsonStore::new(dir),
        }
    }

    pub fn save(&self, analysis: &AdvancedGameAnalysis) -> Result<(), PersistenceError> {
        self.inner.save(analysis)?;
        Ok(())
    }

    pub fn load(&self, game_id: &str) -> Result<Option<AdvancedGameAnalysis>, PersistenceError> {
        self.inner.load(game_id)
    }

    pub fn delete(&self, game_id: &str) -> Result<(), PersistenceError> {
        self.inner.delete(game_id)
    }
}
