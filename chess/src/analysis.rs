//! Engine analysis types shared across server and client.

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
#[derive(Debug, Clone)]
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
}

impl std::fmt::Display for AnalysisScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
