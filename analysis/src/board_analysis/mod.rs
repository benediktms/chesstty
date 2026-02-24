pub mod attack_map;
pub mod back_rank_detector;
pub mod detector;
pub mod discovered_attack_detector;
pub mod fork_detector;
pub mod hanging_detector;
pub mod helpers;
pub mod king_safety;
pub mod mate_threat_detector;
pub mod pin_detector;
pub mod reducer;
pub mod sacrifice_detector;
pub mod skewer_detector;
pub mod tactical_types;
pub mod tension;
pub mod zwischenzug_detector;

pub use attack_map::{AttackMap, Attacker, PinInfo};
pub use detector::{TacticalContext, TacticalDetector};
pub use king_safety::{compute_king_safety, KingSafetyMetrics, PositionKingSafety};
pub use tactical_types::{TacticalEvidence, TacticalLine, TacticalTag, TacticalTagKind};
pub use tension::{compute_tension, PositionTensionMetrics};

use back_rank_detector::BackRankDetector;
use discovered_attack_detector::DiscoveredAttackDetector;
use fork_detector::{DoubleAttackDetector, ForkDetector};
use hanging_detector::HangingPieceDetector;
use mate_threat_detector::MateThreatDetector;
use pin_detector::PinDetector;
use reducer::reduce_tags;
use sacrifice_detector::SacrificeDetector;
use skewer_detector::SkewerDetector;
use zwischenzug_detector::ZwischenzugDetector;

/// Run all tactical detectors on the given context and return deduplicated,
/// ranked tags.
///
/// This is the main entry point for the new tactical detection pipeline.
/// It instantiates every registered `TacticalDetector`, collects their output,
/// and passes the combined tags through `reduce_tags` for deduplication and
/// ranking.
pub fn detect_tactics(ctx: &TacticalContext, max_results: Option<usize>) -> Vec<TacticalTag> {
    let detectors: Vec<Box<dyn TacticalDetector>> = vec![
        Box::new(MateThreatDetector),
        Box::new(ForkDetector),
        Box::new(DoubleAttackDetector),
        Box::new(PinDetector),
        Box::new(SkewerDetector),
        Box::new(DiscoveredAttackDetector),
        Box::new(SacrificeDetector),
        Box::new(HangingPieceDetector),
        Box::new(BackRankDetector),
        Box::new(ZwischenzugDetector),
    ];

    let tags: Vec<TacticalTag> = detectors.iter().flat_map(|d| d.detect(ctx)).collect();

    reduce_tags(tags, max_results)
}

#[cfg(test)]
mod tests {
    use cozy_chess::{Board, Color, Move, Square};

    use super::*;
    use crate::board_analysis::attack_map::AttackMap;
    use crate::board_analysis::detector::TacticalContext;

    fn make_ctx<'a>(
        before: &'a Board,
        after: &'a Board,
        before_attacks: &'a AttackMap,
        after_attacks: &'a AttackMap,
    ) -> TacticalContext<'a> {
        TacticalContext {
            before,
            after,
            mv: None,
            side_to_move_before: before.side_to_move(),
            before_attacks,
            after_attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        }
    }

    // -----------------------------------------------------------------------
    // Starting position: no tactics expected
    // -----------------------------------------------------------------------

    #[test]
    fn starting_position_returns_no_tags() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = make_ctx(&board, &board, &attacks, &attacks);

        let tags = detect_tactics(&ctx, None);
        assert!(
            tags.is_empty(),
            "starting position should produce no tactical tags, got: {:?}",
            tags
        );
    }

    // -----------------------------------------------------------------------
    // max_results truncation
    // -----------------------------------------------------------------------

    #[test]
    fn max_results_truncates_output() {
        // Knight fork: Nf6 attacks black king on e8 and black rook on d7.
        // This produces at least one Fork tag. With max_results=0 we get nothing.
        let board: Board = "4k3/3r4/5N2/8/8/8/8/4K3 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        // perspective = White (the forking side)
        let ctx = TacticalContext {
            before: &board,
            after: &board,
            mv: None,
            side_to_move_before: Color::White,
            before_attacks: &attacks,
            after_attacks: &attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        };

        let all_tags = detect_tactics(&ctx, None);
        assert!(!all_tags.is_empty(), "fork position should produce tags");

        let truncated = detect_tactics(&ctx, Some(1));
        assert_eq!(truncated.len(), 1, "max_results=1 must truncate to 1 tag");

        let zero = detect_tactics(&ctx, Some(0));
        assert!(zero.is_empty(), "max_results=0 must return empty");
    }

    // -----------------------------------------------------------------------
    // Fork detected end-to-end through the pipeline
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_detects_fork() {
        // White knight on f6 forks black king on e8 and black rook on d7.
        let board: Board = "4k3/3r4/5N2/8/8/8/8/4K3 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = TacticalContext {
            before: &board,
            after: &board,
            mv: None,
            side_to_move_before: Color::White,
            before_attacks: &attacks,
            after_attacks: &attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        };

        let tags = detect_tactics(&ctx, None);
        let fork = tags.iter().find(|t| t.kind == TacticalTagKind::Fork);
        assert!(fork.is_some(), "pipeline should detect Fork tag");
        assert_eq!(fork.unwrap().attacker.as_deref(), Some("f6"));
    }

    // -----------------------------------------------------------------------
    // Hanging piece detected end-to-end
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_detects_hanging_piece() {
        // Black knight on d5 is attacked by white bishop on f3, no defenders.
        let board: Board = "4k3/8/8/3n4/8/5B2/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = make_ctx(&board, &board, &attacks, &attacks);

        let tags = detect_tactics(&ctx, None);
        let hanging = tags
            .iter()
            .find(|t| t.kind == TacticalTagKind::HangingPiece);
        assert!(
            hanging.is_some(),
            "pipeline should detect HangingPiece tag for undefended knight on d5"
        );
    }

    // -----------------------------------------------------------------------
    // Back-rank weakness detected end-to-end
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_detects_back_rank_weakness() {
        // White king on g1, pawns f2/g2/h2 block escape, black rook on a8.
        // Black is the attacker (perspective = Black detects weakness in White).
        let board: Board = "r3k3/8/8/8/8/8/5PPP/6K1 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = TacticalContext {
            before: &board,
            after: &board,
            mv: None,
            side_to_move_before: Color::Black,
            before_attacks: &attacks,
            after_attacks: &attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        };

        let tags = detect_tactics(&ctx, None);
        let back_rank = tags
            .iter()
            .find(|t| t.kind == TacticalTagKind::BackRankWeakness);
        assert!(
            back_rank.is_some(),
            "pipeline should detect BackRankWeakness tag"
        );
    }

    // -----------------------------------------------------------------------
    // Checkmate position: MateThreat with confidence 1.0 ranks first
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_ranks_mate_threat_first() {
        // Black king g8 is checkmated: Ra8 controls entire 8th rank,
        // pawns f7/g7/h7 block all escape squares.
        let board: Board = "R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = TacticalContext {
            before: &board,
            after: &board,
            mv: None,
            side_to_move_before: Color::White,
            before_attacks: &attacks,
            after_attacks: &attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        };

        let tags = detect_tactics(&ctx, None);
        assert!(!tags.is_empty(), "checkmate position should produce tags");
        assert_eq!(
            tags[0].kind,
            TacticalTagKind::MateThreat,
            "MateThreat should be ranked first (confidence 1.0)"
        );
        assert_eq!(tags[0].confidence, 1.0);
    }

    // -----------------------------------------------------------------------
    // Discovered attack detected end-to-end
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_detects_discovered_attack() {
        // Before: White bishop on a1, white knight on c3 blocking a1-e5 diagonal,
        //         black queen on e5. After: Knight moves c3->f5, revealing bishop on e5.
        let before: Board = "K6k/8/8/4q3/8/2N5/8/B7 w - - 0 1".parse().unwrap();
        let after: Board = "K6k/8/8/4qN2/8/8/8/B7 b - - 1 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::C3,
            to: Square::F5,
            promotion: None,
        };

        let ctx = TacticalContext {
            before: &before,
            after: &after,
            mv: Some(mv),
            side_to_move_before: Color::White,
            before_attacks: &before_attacks,
            after_attacks: &after_attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        };

        let tags = detect_tactics(&ctx, None);
        let disc = tags
            .iter()
            .find(|t| t.kind == TacticalTagKind::DiscoveredAttack);
        assert!(
            disc.is_some(),
            "pipeline should detect DiscoveredAttack tag, got: {:?}",
            tags
        );
    }
}
