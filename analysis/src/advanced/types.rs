use serde::{Deserialize, Serialize};

use crate::board_analysis::{PositionKingSafety, PositionTensionMetrics, TacticalAnalysis};

/// Advanced analysis result for a single position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedPositionAnalysis {
    pub ply: u32,
    pub tactics_before: TacticalAnalysis,
    pub tactics_after: TacticalAnalysis,
    pub king_safety: PositionKingSafety,
    pub tension: PositionTensionMetrics,
    /// Whether this position was flagged as critical (multi-signal).
    pub is_critical: bool,
    /// If re-analyzed at deeper depth, the depth used.
    pub deep_depth: Option<u32>,
}

/// Psychological profile for one player across a game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsychologicalProfile {
    pub color: char,
    /// Maximum consecutive errors (Inaccuracy/Mistake/Blunder).
    pub max_consecutive_errors: u8,
    /// Ply where the worst error streak started.
    pub error_streak_start_ply: Option<u32>,
    /// Number of eval swings > 100cp favorable to this side.
    pub favorable_swings: u8,
    /// Number of eval swings > 100cp unfavorable to this side.
    pub unfavorable_swings: u8,
    /// Longest consecutive favorable swing streak.
    pub max_momentum_streak: u8,
    /// Max blunder count in a sliding window of 5 same-side moves.
    pub blunder_cluster_density: u8,
    /// Ply range of the densest blunder cluster.
    pub blunder_cluster_range: Option<(u32, u32)>,
    /// Pearson correlation of time-per-move vs cp_loss (if clock data available).
    pub time_quality_correlation: Option<f32>,
    /// Average time spent on blunder moves (ms).
    pub avg_blunder_time_ms: Option<u64>,
    /// Average time spent on good moves (Best/Excellent/Good) (ms).
    pub avg_good_move_time_ms: Option<u64>,
    /// Average cp_loss during opening phase (plies 1-30).
    pub opening_avg_cp_loss: f64,
    /// Average cp_loss during middlegame phase (plies 31-70).
    pub middlegame_avg_cp_loss: f64,
    /// Average cp_loss during endgame phase (plies 71+).
    pub endgame_avg_cp_loss: f64,
}

/// Complete advanced analysis for a game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedGameAnalysis {
    pub game_id: String,
    pub positions: Vec<AdvancedPositionAnalysis>,
    pub white_psychology: PsychologicalProfile,
    pub black_psychology: PsychologicalProfile,
    pub pipeline_version: u32,
    pub shallow_depth: u32,
    pub deep_depth: u32,
    pub critical_positions_count: u32,
    pub computed_at: u64,
}

/// Configuration for the multi-pass analysis pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Phase 1 engine depth (default: 10).
    pub shallow_depth: u32,
    /// Phase 3 deep re-analysis depth (default: 22).
    pub deep_depth: u32,
    /// Maximum number of critical positions to deep-analyze (default: 20).
    pub max_critical_positions: usize,
    /// Whether to compute advanced analysis at all.
    pub compute_advanced: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            shallow_depth: 10,
            deep_depth: 22,
            max_critical_positions: 20,
            compute_advanced: true,
        }
    }
}
