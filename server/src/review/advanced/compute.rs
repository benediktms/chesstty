use analysis::advanced::critical::is_critical_position;
use analysis::advanced::psychological::compute_psychological_profile;
use analysis::advanced::types::{AdvancedGameAnalysis, AdvancedPositionAnalysis, AnalysisConfig};
use analysis::board_analysis::{
    analyze_tactics, compute_king_safety, compute_tension, PositionKingSafety,
    PositionTensionMetrics, TacticalAnalysis,
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

        // Parse FEN for board analysis
        // Use fen_before for tactics_before, fen_after (pos.fen) for tactics_after
        let fen_before = if i == 0 {
            // First move: we don't have the starting FEN in PositionReview,
            // so use the position's FEN as "after" only
            None
        } else {
            // The previous position's FEN is the position after the previous move
            Some(&review.positions[i - 1].fen)
        };

        let board_after = pos.fen.parse::<Board>().ok();

        // Tactics before this move (the position the player was looking at)
        let tactics_before = fen_before
            .and_then(|fen| fen.parse::<Board>().ok())
            .map(|board| {
                let color = board.side_to_move();
                analyze_tactics(&board, color)
            })
            .unwrap_or_else(empty_tactical_analysis);

        // Tactics after this move (the resulting position)
        let tactics_after = board_after
            .as_ref()
            .map(|board| {
                let color = board.side_to_move();
                analyze_tactics(board, color)
            })
            .unwrap_or_else(empty_tactical_analysis);

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
        let is_critical = is_critical_position(
            pos,
            prev_pos,
            &tactics_before,
            &king_safety,
            &tension,
        );

        if is_critical {
            critical_count += 1;
        }

        positions.push(AdvancedPositionAnalysis {
            ply: pos.ply,
            tactics_before,
            tactics_after,
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

fn empty_tactical_analysis() -> TacticalAnalysis {
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
        black: KingSafetyMetrics {
            color: 'b',
            ..m
        },
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
