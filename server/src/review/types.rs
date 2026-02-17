use crate::persistence::Storable;
pub use chess::is_white_ply;
pub use chess::AnalysisScore;
use serde::{Deserialize, Serialize};

/// Classification of a move's quality relative to the engine's best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveClassification {
    /// Better than engine expected (rare tactical find).
    Brilliant,
    /// Matches or very close to engine's best move (0 cp loss).
    Best,
    /// Within 10 cp of best.
    Excellent,
    /// Within 30 cp of best.
    Good,
    /// 31-100 cp worse than best.
    Inaccuracy,
    /// 101-300 cp worse than best.
    Mistake,
    /// 300+ cp worse than best.
    Blunder,
    /// Only one legal move available.
    Forced,
    /// Opening book move (first N plies, optional).
    Book,
}

impl MoveClassification {
    /// Classify based on centipawn loss.
    /// `cp_loss` should be non-negative (how many cp the played move lost).
    pub fn from_cp_loss(cp_loss: i32, is_forced: bool) -> Self {
        if is_forced {
            return Self::Forced;
        }
        match cp_loss {
            i if i <= 0 => Self::Best,
            1..=10 => Self::Excellent,
            11..=30 => Self::Good,
            31..=100 => Self::Inaccuracy,
            101..=300 => Self::Mistake,
            _ => Self::Blunder,
        }
    }

    /// NAG (Numeric Annotation Glyph) for PGN export.
    pub fn to_nag(self) -> Option<u8> {
        match self {
            Self::Brilliant => Some(3), // !!
            Self::Best => None,
            Self::Excellent => Some(1), // !
            Self::Good => None,
            Self::Inaccuracy => Some(6), // ?!
            Self::Mistake => Some(2),    // ?
            Self::Blunder => Some(4),    // ??
            Self::Forced => None,
            Self::Book => None,
        }
    }
}

/// Analysis result for a single position/ply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionReview {
    pub ply: u32,
    pub fen: String,
    pub played_san: String,
    pub best_move_san: String,
    pub best_move_uci: String,
    pub eval_before: AnalysisScore,
    pub eval_after: AnalysisScore,
    pub eval_best: AnalysisScore,
    pub classification: MoveClassification,
    pub cp_loss: i32,
    pub pv: Vec<String>,
    pub depth: u32,
    #[serde(default)]
    pub clock_ms: Option<u64>,
}

/// Status of a review job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewStatus {
    Queued,
    Analyzing { current_ply: u32, total_plies: u32 },
    Complete,
    Failed { error: String },
}

/// Full review result for a game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameReview {
    pub game_id: String,
    pub status: ReviewStatus,
    pub positions: Vec<PositionReview>,
    pub white_accuracy: Option<f64>,
    pub black_accuracy: Option<f64>,
    pub total_plies: u32,
    pub analyzed_plies: u32,
    pub analysis_depth: u32,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

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

/// Compute accuracy percentage for one side.
/// Uses the formula: accuracy = 103.1668 * exp(-0.006 * avg_cp_loss) - 3.1668
/// Clamped to [0, 100].
///
/// The exponential constant (0.006) is calibrated for raw centipawn loss so that
/// accuracy values match typical chess platform norms:
///   ACPL=10 → ~94%, ACPL=35 → ~80%, ACPL=100 → ~54%
///
/// Individual cp_loss values are capped at 1000 to prevent mate-related outliers
/// (where to_cp() returns 20000+) from destroying the average.
pub fn compute_accuracy(positions: &[PositionReview], is_white: bool) -> f64 {
    // Plies are 1-indexed: odd plies (1, 3, 5, ...) are white moves,
    // even plies (2, 4, 6, ...) are black moves.
    let side_positions: Vec<&PositionReview> = positions
        .iter()
        .filter(|p| is_white_ply(p.ply) == is_white)
        .collect();

    if side_positions.is_empty() {
        return 100.0;
    }

    let total_cp_loss: f64 = side_positions
        .iter()
        .map(|p| (p.cp_loss as f64).min(1000.0))
        .sum();
    let avg_cp_loss = total_cp_loss / side_positions.len() as f64;

    let accuracy = 103.1668 * (-0.006 * avg_cp_loss).exp() - 3.1668;
    accuracy.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_best() {
        assert_eq!(
            MoveClassification::from_cp_loss(0, false),
            MoveClassification::Best
        );
        assert_eq!(
            MoveClassification::from_cp_loss(-5, false),
            MoveClassification::Best
        );
    }

    #[test]
    fn test_classification_excellent() {
        assert_eq!(
            MoveClassification::from_cp_loss(5, false),
            MoveClassification::Excellent
        );
        assert_eq!(
            MoveClassification::from_cp_loss(10, false),
            MoveClassification::Excellent
        );
    }

    #[test]
    fn test_classification_good() {
        assert_eq!(
            MoveClassification::from_cp_loss(15, false),
            MoveClassification::Good
        );
        assert_eq!(
            MoveClassification::from_cp_loss(30, false),
            MoveClassification::Good
        );
    }

    #[test]
    fn test_classification_inaccuracy() {
        assert_eq!(
            MoveClassification::from_cp_loss(50, false),
            MoveClassification::Inaccuracy
        );
        assert_eq!(
            MoveClassification::from_cp_loss(100, false),
            MoveClassification::Inaccuracy
        );
    }

    #[test]
    fn test_classification_mistake() {
        assert_eq!(
            MoveClassification::from_cp_loss(150, false),
            MoveClassification::Mistake
        );
        assert_eq!(
            MoveClassification::from_cp_loss(300, false),
            MoveClassification::Mistake
        );
    }

    #[test]
    fn test_classification_blunder() {
        assert_eq!(
            MoveClassification::from_cp_loss(350, false),
            MoveClassification::Blunder
        );
        assert_eq!(
            MoveClassification::from_cp_loss(1000, false),
            MoveClassification::Blunder
        );
    }

    #[test]
    fn test_classification_forced() {
        assert_eq!(
            MoveClassification::from_cp_loss(500, true),
            MoveClassification::Forced
        );
        assert_eq!(
            MoveClassification::from_cp_loss(0, true),
            MoveClassification::Forced
        );
    }

    #[test]
    fn test_stored_score_to_cp() {
        assert_eq!(AnalysisScore::Centipawns(50).to_cp(), 50);
        assert_eq!(AnalysisScore::Centipawns(-100).to_cp(), -100);
        // Mate in 3 should be a very high value
        assert!(AnalysisScore::Mate(3).to_cp() > 10000);
        // Mated in 3 should be a very low value
        assert!(AnalysisScore::Mate(-3).to_cp() < -10000);
    }

    #[test]
    fn test_stored_score_negate() {
        let score = AnalysisScore::Centipawns(50);
        assert_eq!(score.negate().to_cp(), -50);
        let mate = AnalysisScore::Mate(3);
        assert_eq!(mate.negate().to_cp(), AnalysisScore::Mate(-3).to_cp());
    }

    #[test]
    fn test_compute_accuracy_perfect() {
        // No cp loss = near 100% accuracy (ply 1 = white's first move)
        let positions = vec![PositionReview {
            ply: 1,
            fen: String::new(),
            played_san: "e4".into(),
            best_move_san: "e4".into(),
            best_move_uci: "e2e4".into(),
            eval_before: AnalysisScore::Centipawns(20),
            eval_after: AnalysisScore::Centipawns(20),
            eval_best: AnalysisScore::Centipawns(20),
            classification: MoveClassification::Best,
            cp_loss: 0,
            pv: vec![],
            depth: 18,
            clock_ms: None,
        }];
        let accuracy = compute_accuracy(&positions, true);
        assert!(accuracy > 99.0);
    }

    #[test]
    fn test_compute_accuracy_poor() {
        // Large cp loss = low accuracy (ply 1 = white's first move)
        let positions = vec![PositionReview {
            ply: 1,
            fen: String::new(),
            played_san: "f3".into(),
            best_move_san: "e4".into(),
            best_move_uci: "e2e4".into(),
            eval_before: AnalysisScore::Centipawns(20),
            eval_after: AnalysisScore::Centipawns(-200),
            eval_best: AnalysisScore::Centipawns(20),
            classification: MoveClassification::Mistake,
            cp_loss: 200,
            pv: vec![],
            depth: 18,
            clock_ms: None,
        }];
        let accuracy = compute_accuracy(&positions, true);
        assert!(accuracy < 50.0);
    }

    #[test]
    fn test_compute_accuracy_filters_by_side() {
        // Ply 1 (odd) = white, ply 2 (even) = black
        let positions = vec![
            PositionReview {
                ply: 1,
                fen: String::new(),
                played_san: "e4".into(),
                best_move_san: "e4".into(),
                best_move_uci: "e2e4".into(),
                eval_before: AnalysisScore::Centipawns(20),
                eval_after: AnalysisScore::Centipawns(20),
                eval_best: AnalysisScore::Centipawns(20),
                classification: MoveClassification::Best,
                cp_loss: 0,
                pv: vec![],
                depth: 18,
                clock_ms: None,
            },
            PositionReview {
                ply: 2,
                fen: String::new(),
                played_san: "f6".into(),
                best_move_san: "e5".into(),
                best_move_uci: "e7e5".into(),
                eval_before: AnalysisScore::Centipawns(-20),
                eval_after: AnalysisScore::Centipawns(180),
                eval_best: AnalysisScore::Centipawns(-20),
                classification: MoveClassification::Mistake,
                cp_loss: 200,
                pv: vec![],
                depth: 18,
                clock_ms: None,
            },
        ];
        let white_accuracy = compute_accuracy(&positions, true);
        let black_accuracy = compute_accuracy(&positions, false);
        // White played perfectly, black made a mistake
        assert!(white_accuracy > 99.0);
        assert!(black_accuracy < 50.0);
    }
}
