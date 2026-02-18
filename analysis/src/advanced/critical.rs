use crate::board_analysis::{PositionKingSafety, PositionTensionMetrics, TacticalAnalysis};
use crate::review_types::PositionReview;

/// Determine if a position is "critical" based on multiple signals.
/// Returns true if >= 2 of the following signals fire:
/// 1. cp_loss > 50
/// 2. Eval swing > 150cp compared to previous position
/// 3. Tactical motif detected (forks, pins, skewers, etc.)
/// 4. Volatility > 0.6
/// 5. King exposure > 0.7 (either side)
pub fn is_critical_position(
    position: &PositionReview,
    prev_position: Option<&PositionReview>,
    tactics: &TacticalAnalysis,
    king_safety: &PositionKingSafety,
    tension: &PositionTensionMetrics,
) -> bool {
    let mut signals = 0u8;

    // Signal 1: significant cp_loss
    if position.cp_loss > 50 {
        signals += 1;
    }

    // Signal 2: eval swing compared to previous position
    if let Some(prev) = prev_position {
        let prev_eval = prev.eval_after.to_cp();
        let curr_eval = position.eval_after.to_cp();
        if (curr_eval - prev_eval).unsigned_abs() > 150 {
            signals += 1;
        }
    }

    // Signal 3: tactical motif detected
    if !tactics.patterns.is_empty() {
        signals += 1;
    }

    // Signal 4: high volatility
    if tension.volatility_score > 0.6 {
        signals += 1;
    }

    // Signal 5: king exposure (either side)
    if king_safety.white.exposure_score > 0.7 || king_safety.black.exposure_score > 0.7 {
        signals += 1;
    }

    signals >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_analysis::{KingSafetyMetrics, TacticalPattern};
    use chess::AnalysisScore;

    fn make_position_review(cp_loss: i32, eval_after_cp: i32) -> PositionReview {
        PositionReview {
            ply: 1,
            fen: String::new(),
            played_san: String::new(),
            best_move_san: String::new(),
            best_move_uci: String::new(),
            eval_before: AnalysisScore::Centipawns(0),
            eval_after: AnalysisScore::Centipawns(eval_after_cp),
            eval_best: AnalysisScore::Centipawns(0),
            classification: crate::MoveClassification::Best,
            cp_loss,
            pv: vec![],
            depth: 18,
            clock_ms: None,
        }
    }

    fn empty_tactics() -> TacticalAnalysis {
        TacticalAnalysis {
            patterns: vec![],
            fork_count: 0,
            pin_count: 0,
            skewer_count: 0,
            discovered_attack_count: 0,
            hanging_piece_count: 0,
            has_back_rank_weakness: false,
        }
    }

    fn safe_king_safety() -> PositionKingSafety {
        let metrics = KingSafetyMetrics {
            color: 'w',
            pawn_shield_count: 3,
            pawn_shield_max: 3,
            open_files_near_king: 0,
            attacker_count: 0,
            attack_weight: 0,
            attacked_king_zone_squares: 0,
            king_zone_size: 5,
            exposure_score: 0.1,
        };
        PositionKingSafety {
            white: metrics.clone(),
            black: KingSafetyMetrics {
                color: 'b',
                ..metrics
            },
        }
    }

    fn quiet_tension() -> PositionTensionMetrics {
        PositionTensionMetrics {
            mutually_attacked_pairs: 0,
            contested_squares: 5,
            attacked_but_defended: 0,
            forcing_moves: 0,
            checks_available: 0,
            captures_available: 0,
            volatility_score: 0.1,
        }
    }

    #[test]
    fn test_not_critical_quiet_position() {
        let pos = make_position_review(10, 20);
        let result = is_critical_position(
            &pos,
            None,
            &empty_tactics(),
            &safe_king_safety(),
            &quiet_tension(),
        );
        assert!(!result, "Quiet position should not be critical");
    }

    #[test]
    fn test_critical_high_cp_loss_and_tactics() {
        let pos = make_position_review(100, -200);
        let mut tactics = empty_tactics();
        tactics.patterns.push(TacticalPattern::HangingPiece {
            piece: crate::board_analysis::SquareInfo {
                square: "d5".into(),
                piece: 'N',
                color: 'b',
            },
            attacker_count: 1,
            defender_count: 0,
        });
        tactics.hanging_piece_count = 1;

        let result = is_critical_position(
            &pos,
            None,
            &tactics,
            &safe_king_safety(),
            &quiet_tension(),
        );
        assert!(result, "High cp_loss + tactics should be critical");
    }

    #[test]
    fn test_critical_eval_swing_and_volatility() {
        let prev = make_position_review(0, 100);
        let pos = make_position_review(0, -200);
        let mut tension = quiet_tension();
        tension.volatility_score = 0.8;

        let result = is_critical_position(
            &pos,
            Some(&prev),
            &empty_tactics(),
            &safe_king_safety(),
            &tension,
        );
        assert!(result, "Eval swing + high volatility should be critical");
    }
}
