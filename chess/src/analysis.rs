//! Engine analysis types shared across server and client.

use serde::{Deserialize, Serialize};

/// A snapshot of engine analysis output.
#[derive(Debug, Clone, Default)]
pub struct EngineAnalysis {
    pub depth: Option<u32>,
    pub seldepth: Option<u32>,
    pub time_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub score: Option<AnalysisScore>,
    /// Principal variation as UCI move strings.
    pub pv: Vec<String>,
    pub nps: Option<u64>,
}

/// Engine evaluation score.
///
/// Centipawns: positive = side-to-move is better.
/// Mate: positive N = side-to-move mates in N moves,
/// negative N = side-to-move gets mated in N moves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisScore {
    Centipawns(i32),
    Mate(i32),
}

impl AnalysisScore {
    pub fn display(&self) -> String {
        match self {
            Self::Centipawns(cp) => format!("{:+.2}", *cp as f64 / 100.0),
            Self::Mate(m) => {
                if *m > 0 {
                    format!("+M{}", m)
                } else {
                    format!("-M{}", m.abs())
                }
            }
        }
    }

    /// Convert to centipawns for comparison. Mate scores use large values.
    pub fn to_cp(&self) -> i32 {
        match self {
            Self::Centipawns(cp) => *cp,
            Self::Mate(m) => {
                if *m > 0 {
                    30000 - *m * 100
                } else {
                    -30000 - *m * 100
                }
            }
        }
    }

    /// Negate the score (flip perspective).
    pub fn negate(&self) -> Self {
        match self {
            Self::Centipawns(cp) => Self::Centipawns(-cp),
            Self::Mate(m) => Self::Mate(-m),
        }
    }
}

/// Returns true if the given 1-indexed ply belongs to White.
/// Convention: odd plies (1, 3, 5, …) are White moves; even plies (2, 4, 6, …) are Black.
pub fn is_white_ply(ply: u32) -> bool {
    ply % 2 == 1
}

impl std::fmt::Display for AnalysisScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
