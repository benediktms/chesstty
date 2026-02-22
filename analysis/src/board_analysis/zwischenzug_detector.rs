use super::detector::{TacticalContext, TacticalDetector};
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects potential Zwischenzug (intermediate moves): a move that gives check
/// when a different first move is suggested by the engine's best line.
///
/// Real zwischenzug detection requires full game history. This heuristic
/// identifies moves that give check as potential zwischenzugs when the engine's
/// best line suggests a different first move.
pub struct ZwischenzugDetector;

impl TacticalDetector for ZwischenzugDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let mv = match ctx.mv {
            Some(m) => m,
            None => return vec![],
        };

        let best_line = match ctx.best_line {
            Some(line) => line,
            None => return vec![],
        };

        // Check if the move gives check (opponent king is in check after the move)
        let gives_check = !ctx.after.checkers().is_empty();
        if !gives_check {
            return vec![];
        }

        // Find the king square of the side that is now in check
        let checked_side = !ctx.side_to_move_before;
        let king_sq = ctx
            .after
            .king(checked_side);

        // Check if the played move differs from the engine's best line suggestion
        let mv_str = format!("{}{}", mv.from, mv.to);
        let best_move_differs = best_line
            .first()
            .is_none_or(|best| !best.starts_with(&mv_str));

        if !best_move_differs {
            return vec![];
        }

        let moved_piece = ctx.before.piece_on(mv.from).unwrap_or(cozy_chess::Piece::Pawn);

        vec![TacticalTag {
            kind: TacticalTagKind::Zwischenzug,
            attacker: Some(mv.to.to_string()),
            victims: vec![king_sq.to_string()],
            target_square: Some(king_sq.to_string()),
            confidence: 0.5,
            note: Some(format!(
                "potential zwischenzug: {} to {} gives check",
                moved_piece, mv.to
            )),
            evidence: TacticalEvidence {
                lines: vec![],
                threatened_pieces: vec![king_sq.to_string()],
                defended_by: vec![],
            },
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
        best_line: Option<&'a [String]>,
    ) -> TacticalContext<'a> {
        TacticalContext {
            before,
            after,
            mv,
            side_to_move_before: perspective,
            before_attacks,
            after_attacks,
            eval_before: None,
            eval_after: None,
            best_line,
        }
    }

    #[test]
    fn detects_zwischenzug_check() {
        // White rook on e1, white moves rook from e1 to e8 giving check to black king on e8.
        // Before: white rook on e1, black king on e8, white king on a1.
        // After: white rook on e8, black king must move.
        // best_line suggests a different move ("d2d4") not the rook check.
        let before: Board = "4k3/8/8/8/8/8/8/R3K3 w Q - 0 1".parse().unwrap();
        // After Rook a1->a8: black king on e8 is in check from rook on a8
        let after: Board = "R3k3/8/8/8/8/8/8/4K3 b - - 1 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::A1,
            to: Square::A8,
            promotion: None,
        };

        let best_line = vec!["d2d4".to_string(), "d7d5".to_string()];

        let ctx = make_ctx(
            &before,
            &after,
            &before_attacks,
            &after_attacks,
            Some(mv),
            Color::White,
            Some(&best_line),
        );

        let tags = ZwischenzugDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::Zwischenzug);
        assert_eq!(tags[0].confidence, 0.5);
        assert_eq!(tags[0].attacker.as_deref(), Some("a8"));
        assert!(tags[0].note.as_ref().unwrap().contains("zwischenzug"));
        assert!(tags[0].note.as_ref().unwrap().contains("check"));
    }

    #[test]
    fn no_zwischenzug_without_move() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let best_line: Vec<String> = vec!["e2e4".to_string()];

        let ctx = make_ctx(
            &board,
            &board,
            &attacks,
            &attacks,
            None,
            Color::White,
            Some(&best_line),
        );

        let tags = ZwischenzugDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn no_zwischenzug_without_best_line() {
        let before: Board = "R3k3/8/8/8/8/8/8/4K3 b - - 1 1".parse().unwrap();
        let attacks = AttackMap::compute(&before);

        let mv = Move {
            from: Square::A1,
            to: Square::A8,
            promotion: None,
        };

        let ctx = make_ctx(
            &before,
            &before,
            &attacks,
            &attacks,
            Some(mv),
            Color::White,
            None,
        );

        let tags = ZwischenzugDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn no_zwischenzug_when_move_matches_best_line() {
        // White rook gives check, and the best_line also suggests that same move.
        let before: Board = "4k3/8/8/8/8/8/8/R3K3 w Q - 0 1".parse().unwrap();
        let after: Board = "R3k3/8/8/8/8/8/8/4K3 b - - 1 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::A1,
            to: Square::A8,
            promotion: None,
        };

        // Best line matches the played move exactly
        let best_line = vec!["a1a8".to_string()];

        let ctx = make_ctx(
            &before,
            &after,
            &before_attacks,
            &after_attacks,
            Some(mv),
            Color::White,
            Some(&best_line),
        );

        let tags = ZwischenzugDetector.detect(&ctx);
        assert!(
            tags.is_empty(),
            "should not tag a move that matches the best line"
        );
    }
}
