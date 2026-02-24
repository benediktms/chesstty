use crate::persistence::{JsonStore, PersistenceError, Storable};
use analysis::AdvancedGameAnalysis;
use std::path::PathBuf;

impl Storable for AdvancedGameAnalysis {
    fn id(&self) -> &str {
        &self.game_id
    }
}

/// Persistence layer for advanced game analyses.
/// Kept as a fallback trait implementation; production uses SqliteAdvancedAnalysisRepository.
#[allow(dead_code)]
pub struct AdvancedAnalysisStore {
    inner: JsonStore<AdvancedGameAnalysis>,
}

#[allow(dead_code)]
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

impl crate::persistence::traits::AdvancedAnalysisRepository for AdvancedAnalysisStore {
    async fn save_analysis(
        &self,
        analysis: &analysis::AdvancedGameAnalysis,
    ) -> Result<(), PersistenceError> {
        self.save(analysis)
    }

    async fn load_analysis(
        &self,
        game_id: &str,
    ) -> Result<Option<analysis::AdvancedGameAnalysis>, PersistenceError> {
        self.load(game_id)
    }

    async fn delete_analysis(&self, game_id: &str) -> Result<(), PersistenceError> {
        self.delete(game_id)
    }
}
