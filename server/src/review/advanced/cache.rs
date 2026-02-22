use chess::AnalysisScore;
use std::collections::HashMap;

/// Cache entry for a position evaluation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheEntry {
    pub score: AnalysisScore,
    pub best_move_uci: String,
    pub pv: Vec<String>,
    pub depth: u32,
}

/// In-memory cache for engine evaluations keyed by FEN.
/// Returns entries only if cached at >= requested depth.
#[allow(dead_code)]
pub struct EvalCache {
    entries: HashMap<String, CacheEntry>,
}

#[allow(dead_code)]
impl EvalCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert or update an evaluation. Only overwrites if new depth >= existing.
    pub fn insert(&mut self, fen: String, entry: CacheEntry) {
        match self.entries.get(&fen) {
            Some(existing) if existing.depth >= entry.depth => {
                // Existing entry is deeper or equal; keep it.
            }
            _ => {
                self.entries.insert(fen, entry);
            }
        }
    }

    /// Get a cached evaluation, but only if it was computed at >= `min_depth`.
    pub fn get(&self, fen: &str, min_depth: u32) -> Option<&CacheEntry> {
        self.entries.get(fen).filter(|e| e.depth >= min_depth)
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut cache = EvalCache::new();
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";

        cache.insert(
            fen.to_string(),
            CacheEntry {
                score: AnalysisScore::Centipawns(30),
                best_move_uci: "e7e5".into(),
                pv: vec!["e7e5".into()],
                depth: 10,
            },
        );

        // Should find at depth <= 10
        assert!(cache.get(fen, 10).is_some());
        assert!(cache.get(fen, 5).is_some());

        // Should not find at depth > 10
        assert!(cache.get(fen, 15).is_none());
    }

    #[test]
    fn test_deeper_entry_overwrites() {
        let mut cache = EvalCache::new();
        let fen = "start";

        cache.insert(
            fen.to_string(),
            CacheEntry {
                score: AnalysisScore::Centipawns(10),
                best_move_uci: "e2e4".into(),
                pv: vec![],
                depth: 5,
            },
        );

        cache.insert(
            fen.to_string(),
            CacheEntry {
                score: AnalysisScore::Centipawns(20),
                best_move_uci: "d2d4".into(),
                pv: vec![],
                depth: 15,
            },
        );

        let entry = cache.get(fen, 1).unwrap();
        assert_eq!(entry.depth, 15);
        assert_eq!(entry.best_move_uci, "d2d4");
    }

    #[test]
    fn test_shallower_entry_does_not_overwrite() {
        let mut cache = EvalCache::new();
        let fen = "start";

        cache.insert(
            fen.to_string(),
            CacheEntry {
                score: AnalysisScore::Centipawns(20),
                best_move_uci: "d2d4".into(),
                pv: vec![],
                depth: 15,
            },
        );

        cache.insert(
            fen.to_string(),
            CacheEntry {
                score: AnalysisScore::Centipawns(10),
                best_move_uci: "e2e4".into(),
                pv: vec![],
                depth: 5,
            },
        );

        let entry = cache.get(fen, 1).unwrap();
        assert_eq!(entry.depth, 15);
        assert_eq!(entry.best_move_uci, "d2d4");
    }
}
