use cozy_chess::{Board, Color, Move};

use super::attack_map::AttackMap;
use super::tactical_types::TacticalTag;

/// Pre-computed context passed to every tactical detector.
///
/// Carries both the position before and after the move, pre-computed attack maps
/// for each, and optional engine evaluation data. Detectors should treat this as
/// read-only and produce zero or more `TacticalTag` values from it.
pub struct TacticalContext<'a> {
    /// Position before the move was played.
    pub before: &'a Board,
    /// Position after the move was played.
    pub after: &'a Board,
    /// The move that was played, if available.
    pub mv: Option<Move>,
    /// Side to move in the *before* position.
    pub side_to_move_before: Color,
    /// Attack map for the *before* position.
    pub before_attacks: &'a AttackMap,
    /// Attack map for the *after* position.
    pub after_attacks: &'a AttackMap,
    /// Engine centipawn evaluation of the *before* position (from white's perspective).
    pub eval_before: Option<i32>,
    /// Engine centipawn evaluation of the *after* position (from white's perspective).
    pub eval_after: Option<i32>,
    /// Principal variation / best line from the engine, if available.
    pub best_line: Option<&'a [String]>,
}

/// A modular tactical pattern detector.
///
/// Each detector is a zero-sized struct that inspects a `TacticalContext` and
/// returns any tactical tags it finds. The pipeline calls `detect()` on every
/// registered detector and collects the results.
pub trait TacticalDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag>;
}

#[cfg(test)]
mod tests {
    use cozy_chess::{Board, Color};

    use super::*;
    use crate::board_analysis::attack_map::AttackMap;
    use crate::board_analysis::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

    /// A trivial detector that always emits a single tag — used to verify the
    /// trait contract works end-to-end.
    struct AlwaysForkDetector;

    impl TacticalDetector for AlwaysForkDetector {
        fn detect(&self, _ctx: &TacticalContext) -> Vec<TacticalTag> {
            vec![TacticalTag {
                kind: TacticalTagKind::Fork,
                attacker: Some("d5".into()),
                victims: vec!["c3".into(), "f6".into()],
                target_square: None,
                confidence: 0.95,
                note: Some("test fork".into()),
                evidence: TacticalEvidence::default(),
            }]
        }
    }

    /// A detector that returns nothing.
    struct NullDetector;

    impl TacticalDetector for NullDetector {
        fn detect(&self, _ctx: &TacticalContext) -> Vec<TacticalTag> {
            vec![]
        }
    }

    fn make_context<'a>(
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

    #[test]
    fn trait_impl_returns_tags() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = make_context(&board, &board, &attacks, &attacks);

        let detector = AlwaysForkDetector;
        let tags = detector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::Fork);
    }

    #[test]
    fn null_detector_returns_empty() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = make_context(&board, &board, &attacks, &attacks);

        let detector = NullDetector;
        let tags = detector.detect(&ctx);

        assert!(tags.is_empty());
    }

    #[test]
    fn context_carries_eval_and_best_line() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let best_line = vec!["e2e4".to_string(), "e7e5".to_string()];

        let ctx = TacticalContext {
            before: &board,
            after: &board,
            mv: None,
            side_to_move_before: Color::White,
            before_attacks: &attacks,
            after_attacks: &attacks,
            eval_before: Some(30),
            eval_after: Some(-15),
            best_line: Some(&best_line),
        };

        assert_eq!(ctx.eval_before, Some(30));
        assert_eq!(ctx.eval_after, Some(-15));
        assert_eq!(ctx.best_line.unwrap().len(), 2);
    }

    #[test]
    fn pipeline_collects_from_multiple_detectors() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = make_context(&board, &board, &attacks, &attacks);

        let detectors: Vec<Box<dyn TacticalDetector>> = vec![
            Box::new(AlwaysForkDetector),
            Box::new(NullDetector),
            Box::new(AlwaysForkDetector),
        ];

        let tags: Vec<TacticalTag> = detectors
            .iter()
            .flat_map(|d| d.detect(&ctx))
            .collect();

        assert_eq!(tags.len(), 2);
    }
}
