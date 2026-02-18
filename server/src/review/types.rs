// Thin shim — re-export everything from the analysis crate
pub use analysis::{
    compute_accuracy, AnalysisScore, GameReview, MoveClassification, PositionReview, ReviewStatus,
    is_white_ply,
};

use crate::persistence::Storable;

// Storable impl stays here (local trait, foreign type)
impl Storable for GameReview {
    fn id(&self) -> &str {
        &self.game_id
    }
}

/// A job submitted to the review queue.
#[derive(Debug, Clone)]
pub struct ReviewJob {
    pub game_id: String,
    pub game_data: crate::persistence::FinishedGameData,
}
