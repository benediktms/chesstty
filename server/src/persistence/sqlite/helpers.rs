//! Shared encode/decode helpers for SQLite ↔ domain type conversions.
//!
//! These functions bridge the gap between domain enums and the string/integer
//! columns used in the SQLite schema's CHECK constraints.

use analysis::{AnalysisScore, MoveClassification, ReviewStatus};

// ── AnalysisScore ──────────────────────────────────────────────────────

/// Encode an `AnalysisScore` into `(type_str, value)` for SQLite columns
/// like `eval_before_type` / `eval_before_value`.
pub fn encode_score(score: &AnalysisScore) -> (&'static str, i32) {
    match score {
        AnalysisScore::Centipawns(v) => ("cp", *v),
        AnalysisScore::Mate(v) => ("mate", *v),
    }
}

/// Decode a `(type_str, value)` pair from SQLite back into an `AnalysisScore`.
pub fn decode_score(type_str: &str, value: i32) -> AnalysisScore {
    match type_str {
        "mate" => AnalysisScore::Mate(value),
        _ => AnalysisScore::Centipawns(value),
    }
}

// ── ReviewStatus ───────────────────────────────────────────────────────

/// Encode a `ReviewStatus` into the columns:
/// `(status, current_ply, total_plies, error)`.
pub fn encode_status(
    status: &ReviewStatus,
) -> (&'static str, Option<u32>, Option<u32>, Option<&str>) {
    match status {
        ReviewStatus::Queued => ("Queued", None, None, None),
        ReviewStatus::Analyzing {
            current_ply,
            total_plies,
        } => ("Analyzing", Some(*current_ply), Some(*total_plies), None),
        ReviewStatus::Complete => ("Complete", None, None, None),
        ReviewStatus::Failed { error } => ("Failed", None, None, Some(error.as_str())),
    }
}

/// Decode SQLite columns back into a `ReviewStatus`.
pub fn decode_status(
    status: &str,
    current_ply: Option<u32>,
    total_plies: Option<u32>,
    error: Option<String>,
) -> ReviewStatus {
    match status {
        "Analyzing" => ReviewStatus::Analyzing {
            current_ply: current_ply.unwrap_or(0),
            total_plies: total_plies.unwrap_or(0),
        },
        "Failed" => ReviewStatus::Failed {
            error: error.unwrap_or_default(),
        },
        "Complete" => ReviewStatus::Complete,
        _ => ReviewStatus::Queued,
    }
}

// ── MoveClassification ─────────────────────────────────────────────────

/// Encode a `MoveClassification` to the string used in the SQLite CHECK.
pub fn encode_classification(c: &MoveClassification) -> &'static str {
    match c {
        MoveClassification::Brilliant => "Brilliant",
        MoveClassification::Best => "Best",
        MoveClassification::Excellent => "Excellent",
        MoveClassification::Good => "Good",
        MoveClassification::Inaccuracy => "Inaccuracy",
        MoveClassification::Mistake => "Mistake",
        MoveClassification::Blunder => "Blunder",
        MoveClassification::Forced => "Forced",
        MoveClassification::Book => "Book",
    }
}

/// Decode a classification string from SQLite back into a `MoveClassification`.
pub fn decode_classification(s: &str) -> MoveClassification {
    match s {
        "Brilliant" => MoveClassification::Brilliant,
        "Best" => MoveClassification::Best,
        "Excellent" => MoveClassification::Excellent,
        "Good" => MoveClassification::Good,
        "Inaccuracy" => MoveClassification::Inaccuracy,
        "Mistake" => MoveClassification::Mistake,
        "Blunder" => MoveClassification::Blunder,
        "Forced" => MoveClassification::Forced,
        "Book" => MoveClassification::Book,
        _ => MoveClassification::Good, // safe fallback
    }
}

// ── game_mode normalization ────────────────────────────────────────────

/// Normalize a game_mode string for SQLite storage.
///
/// The JSON stores encode `HumanVsEngine` as `"HumanVsEngine:White"` or
/// `"HumanVsEngine:Black"`. The SQLite schema CHECK only allows the base
/// mode string, with `human_side` stored in a separate column.
pub fn normalize_game_mode(game_mode: &str) -> &str {
    if let Some(base) = game_mode.split(':').next() {
        base
    } else {
        game_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_roundtrip() {
        let (t, v) = encode_score(&AnalysisScore::Centipawns(150));
        assert!(matches!(decode_score(t, v), AnalysisScore::Centipawns(150)));

        let (t, v) = encode_score(&AnalysisScore::Centipawns(-42));
        assert!(matches!(decode_score(t, v), AnalysisScore::Centipawns(-42)));

        let (t, v) = encode_score(&AnalysisScore::Mate(3));
        assert!(matches!(decode_score(t, v), AnalysisScore::Mate(3)));

        let (t, v) = encode_score(&AnalysisScore::Mate(-1));
        assert!(matches!(decode_score(t, v), AnalysisScore::Mate(-1)));
    }

    #[test]
    fn status_roundtrip() {
        let cases = vec![
            ReviewStatus::Queued,
            ReviewStatus::Analyzing {
                current_ply: 5,
                total_plies: 40,
            },
            ReviewStatus::Complete,
            ReviewStatus::Failed {
                error: "engine crashed".to_string(),
            },
        ];
        for status in &cases {
            let (s, cp, tp, e) = encode_status(status);
            let decoded = decode_status(s, cp, tp, e.map(|s| s.to_string()));
            assert_eq!(format!("{:?}", decoded), format!("{:?}", status));
        }
    }

    #[test]
    fn classification_roundtrip() {
        let all = vec![
            MoveClassification::Brilliant,
            MoveClassification::Best,
            MoveClassification::Excellent,
            MoveClassification::Good,
            MoveClassification::Inaccuracy,
            MoveClassification::Mistake,
            MoveClassification::Blunder,
            MoveClassification::Forced,
            MoveClassification::Book,
        ];
        for c in &all {
            let s = encode_classification(c);
            let decoded = decode_classification(s);
            assert_eq!(format!("{:?}", decoded), format!("{:?}", c));
        }
    }

    #[test]
    fn normalize_game_mode_strips_side() {
        assert_eq!(normalize_game_mode("HumanVsEngine:White"), "HumanVsEngine");
        assert_eq!(normalize_game_mode("HumanVsEngine:Black"), "HumanVsEngine");
        assert_eq!(normalize_game_mode("HumanVsHuman"), "HumanVsHuman");
        assert_eq!(normalize_game_mode("Analysis"), "Analysis");
    }
}
