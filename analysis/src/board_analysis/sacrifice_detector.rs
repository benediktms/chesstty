use super::detector::{TacticalContext, TacticalDetector};
use super::helpers::piece_value;
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects sacrifices: a piece is given up (material loss) but the evaluation
/// doesn't drop significantly, suggesting the sacrifice is sound.
pub struct SacrificeDetector;

impl TacticalDetector for SacrificeDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let (eval_before, eval_after) = match (ctx.eval_before, ctx.eval_after) {
            (Some(b), Some(a)) => (b, a),
            _ => return vec![],
        };

        let mv = match ctx.mv {
            Some(m) => m,
            None => return vec![],
        };

        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;

        // Determine if this is a sacrifice candidate.
        // Case 1: Moving piece captures a lower-value enemy piece (giving something up for less)
        // Case 2: Moving piece lands on a square attacked by a lower-value enemy piece
        let Some(moving_piece) = ctx.before.piece_on(mv.from) else {
            return vec![];
        };

        let is_sacrifice_candidate = {
            // Case 1: capture of lesser-valued piece
            let capture_sacrifice = ctx.before.piece_on(mv.to).is_some_and(|captured| {
                ctx.before.colors(enemy).has(mv.to)
                    && piece_value(moving_piece) > piece_value(captured)
            });

            // Case 2: moved to square attacked by a lower-value enemy piece
            let move_into_attack = {
                let lowest_attacker_value = ctx
                    .after_attacks
                    .attackers_of(mv.to, enemy)
                    .iter()
                    .map(|a| piece_value(a.piece))
                    .min();
                match lowest_attacker_value {
                    Some(v) => piece_value(moving_piece) > v,
                    None => false,
                }
            };

            capture_sacrifice || move_into_attack
        };

        if !is_sacrifice_candidate {
            return vec![];
        }

        // Eval is from white's perspective. Convert to side's perspective.
        let eval_delta = if perspective == cozy_chess::Color::White {
            eval_after - eval_before
        } else {
            eval_before - eval_after
        };

        // If eval doesn't drop by more than 100cp, it's likely a sound sacrifice.
        if eval_delta < -100 {
            return vec![];
        }

        vec![TacticalTag {
            kind: TacticalTagKind::Sacrifice,
            attacker: Some(mv.to.to_string()),
            victims: vec![],
            target_square: Some(mv.to.to_string()),
            confidence: 0.6,
            note: Some(format!(
                "sacrifice: {} to {} (eval change: {}cp)",
                moving_piece, mv.to, eval_delta
            )),
            evidence: TacticalEvidence::default(),
        }]
    }
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
        mv: Option<Move>,
        perspective: Color,
        eval_before: Option<i32>,
        eval_after: Option<i32>,
    ) -> TacticalContext<'a> {
        TacticalContext {
            before,
            after,
            mv,
            side_to_move_before: perspective,
            before_attacks,
            after_attacks,
            eval_before,
            eval_after,
            best_line: None,
        }
    }

    #[test]
    fn detects_sacrifice_with_eval() {
        // White queen on d5 moves to f7, capturing a pawn (low value).
        // The f7 square is defended by the black king on e8 (which can recapture).
        // Before: white queen on d5, black pawn on f7, black king on e8.
        // After: white queen on f7 (captured pawn), black king on e8 can recapture.
        // Eval stays roughly equal → sound sacrifice.
        let before: Board = "4k3/5p2/8/3Q4/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let after: Board = "4k3/5Q2/8/8/8/8/8/4K3 b - - 0 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::D5,
            to: Square::F7,
            promotion: None,
        };

        // eval_before=50 (white slightly better), eval_after=30 (still fine, small drop)
        let ctx = make_ctx(
            &before,
            &after,
            &before_attacks,
            &after_attacks,
            Some(mv),
            Color::White,
            Some(50),
            Some(30),
        );

        let tags = SacrificeDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::Sacrifice);
        assert_eq!(tags[0].confidence, 0.6);
        assert_eq!(tags[0].target_square.as_deref(), Some("f7"));
        assert!(tags[0].note.as_ref().unwrap().contains("sacrifice"));
    }

    #[test]
    fn no_sacrifice_without_eval() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);

        let mv = Move {
            from: Square::E2,
            to: Square::E4,
            promotion: None,
        };

        let ctx = make_ctx(
            &board,
            &board,
            &attacks,
            &attacks,
            Some(mv),
            Color::White,
            None,
            None,
        );

        let tags = SacrificeDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn no_sacrifice_when_eval_drops_significantly() {
        // If eval drops more than 100cp for the moving side, not a sound sacrifice.
        let before: Board = "4k3/5p2/8/3Q4/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let after: Board = "4k3/5Q2/8/8/8/8/8/4K3 b - - 0 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::D5,
            to: Square::F7,
            promotion: None,
        };

        // Large eval drop from white's perspective → not sound
        let ctx = make_ctx(
            &before,
            &after,
            &before_attacks,
            &after_attacks,
            Some(mv),
            Color::White,
            Some(200),
            Some(-500),
        );

        let tags = SacrificeDetector.detect(&ctx);
        assert!(tags.is_empty());
    }
}
