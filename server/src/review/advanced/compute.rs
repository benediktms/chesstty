use analysis::advanced::critical::is_critical_position;
use analysis::advanced::psychological::compute_psychological_profile;
use analysis::advanced::types::{AdvancedGameAnalysis, AdvancedPositionAnalysis, AnalysisConfig};
use analysis::board_analysis::{
    compute_king_safety, compute_tension, detect_tactics, AttackMap, PositionKingSafety,
    PositionTensionMetrics, TacticalContext, TacticalTag,
};
use analysis::review_types::GameReview;
use cozy_chess::Board;

/// Compute advanced analysis for all positions in a completed review.
/// This is Phase 2 + Phase 4 of the pipeline (pure board geometry, no engine).
pub fn compute_advanced_analysis(
    review: &GameReview,
    config: &AnalysisConfig,
    computed_at: u64,
) -> AdvancedGameAnalysis {
    let mut positions = Vec::with_capacity(review.positions.len());
    let mut critical_count: u32 = 0;

    for (i, pos) in review.positions.iter().enumerate() {
        let prev_pos = if i > 0 {
            Some(&review.positions[i - 1])
        } else {
            None
        };

        let board_before = if i > 0 {
            review.positions[i - 1].fen.parse::<Board>().ok()
        } else {
            None
        };
        let board_after = pos.fen.parse::<Board>().ok();

        // New pipeline: tactical tags
        let tactical_tags_before = detect_for_position(board_before.as_ref());

        let tactical_tags_after = match (board_before.as_ref(), board_after.as_ref()) {
            (Some(before), Some(after)) => {
                let before_attacks = AttackMap::compute(before);
                let after_attacks = AttackMap::compute(after);
                let ctx = TacticalContext {
                    before,
                    after,
                    mv: None,
                    side_to_move_before: before.side_to_move(),
                    before_attacks: &before_attacks,
                    after_attacks: &after_attacks,
                    eval_before: Some(pos.eval_before.to_cp()),
                    eval_after: Some(pos.eval_after.to_cp()),
                    best_line: if pos.pv.is_empty() {
                        None
                    } else {
                        Some(&pos.pv)
                    },
                };
                detect_tactics(&ctx, None)
            }
            (None, Some(after)) => detect_for_position(Some(after)),
            _ => vec![],
        };

        // King safety (from the resulting position)
        let king_safety = board_after
            .as_ref()
            .map(|board| compute_king_safety(board))
            .unwrap_or_else(empty_king_safety);

        // Position tension (from the resulting position)
        let tension = board_after
            .as_ref()
            .map(|board| compute_tension(board))
            .unwrap_or_else(empty_tension);

        // Critical position detection
        let is_critical =
            is_critical_position(pos, prev_pos, &tactical_tags_after, &king_safety, &tension);

        if is_critical {
            critical_count += 1;
        }

        positions.push(AdvancedPositionAnalysis {
            ply: pos.ply,
            tactical_tags_before,
            tactical_tags_after,
            king_safety,
            tension,
            is_critical,
            deep_depth: None,
        });
    }

    // Compute psychological profiles
    let white_psychology = compute_psychological_profile(&review.positions, true);
    let black_psychology = compute_psychological_profile(&review.positions, false);

    AdvancedGameAnalysis {
        game_id: review.game_id.clone(),
        positions,
        white_psychology,
        black_psychology,
        pipeline_version: 1,
        shallow_depth: config.shallow_depth,
        deep_depth: config.deep_depth,
        critical_positions_count: critical_count,
        computed_at,
    }
}

/// Run detectors on a single position (static analysis, no move context).
fn detect_for_position(board: Option<&Board>) -> Vec<TacticalTag> {
    match board {
        Some(b) => {
            let attacks = AttackMap::compute(b);
            let ctx = TacticalContext {
                before: b,
                after: b,
                mv: None,
                side_to_move_before: b.side_to_move(),
                before_attacks: &attacks,
                after_attacks: &attacks,
                eval_before: None,
                eval_after: None,
                best_line: None,
            };
            detect_tactics(&ctx, None)
        }
        None => vec![],
    }
}

fn empty_king_safety() -> PositionKingSafety {
    use analysis::board_analysis::KingSafetyMetrics;
    let m = KingSafetyMetrics {
        color: 'w',
        pawn_shield_count: 0,
        pawn_shield_max: 3,
        open_files_near_king: 0,
        attacker_count: 0,
        attack_weight: 0,
        attacked_king_zone_squares: 0,
        king_zone_size: 0,
        exposure_score: 0.0,
    };
    PositionKingSafety {
        white: m.clone(),
        black: KingSafetyMetrics { color: 'b', ..m },
    }
}

fn empty_tension() -> PositionTensionMetrics {
    PositionTensionMetrics {
        mutually_attacked_pairs: 0,
        contested_squares: 0,
        attacked_but_defended: 0,
        forcing_moves: 0,
        checks_available: 0,
        captures_available: 0,
        volatility_score: 0.0,
    }
}
