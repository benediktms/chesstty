use cozy_chess::Piece;

use super::detector::{TacticalContext, TacticalDetector};
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects hanging pieces: enemy pieces that are attacked but not adequately defended.
pub struct HangingPieceDetector;

impl TacticalDetector for HangingPieceDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;

        let mut tags = Vec::new();

        // Skip pawns (too noisy) and king
        for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
            let targets = ctx.after.pieces(piece) & ctx.after.colors(enemy);
            for sq in targets {
                let our_attackers = ctx.after_attacks.attackers_of(sq, perspective);
                let their_defenders = ctx.after_attacks.attackers_of(sq, enemy);

                let attacker_count = our_attackers.len();
                let defender_count = their_defenders.len();

                if attacker_count == 0 {
                    continue;
                }

                let (confidence, label) = if defender_count == 0 {
                    (0.95, "undefended")
                } else if attacker_count > defender_count {
                    (0.7, "under-defended")
                } else {
                    continue;
                };

                let defender_squares: Vec<String> =
                    their_defenders.iter().map(|a| a.from.to_string()).collect();

                tags.push(TacticalTag {
                    kind: TacticalTagKind::HangingPiece,
                    attacker: None,
                    victims: vec![sq.to_string()],
                    target_square: Some(sq.to_string()),
                    confidence,
                    note: Some(format!(
                        "hanging {} on {}: {} attackers, {} defenders ({})",
                        piece, sq, attacker_count, defender_count, label
                    )),
                    evidence: TacticalEvidence {
                        lines: vec![],
                        threatened_pieces: vec![sq.to_string()],
                        defended_by: defender_squares,
                    },
                });
            }
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use cozy_chess::{Board, Color};

    use super::*;
    use crate::board_analysis::attack_map::AttackMap;
    use crate::board_analysis::detector::TacticalContext;

    fn ctx_from_after<'a>(
        board: &'a Board,
        attacks: &'a AttackMap,
        perspective: Color,
    ) -> TacticalContext<'a> {
        TacticalContext {
            before: board,
            after: board,
            mv: None,
            side_to_move_before: perspective,
            before_attacks: attacks,
            after_attacks: attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        }
    }

    #[test]
    fn detects_undefended_knight() {
        // Black knight on d5, white bishop on f3 attacks it, no black defenders
        let board: Board = "4k3/8/8/3n4/8/5B2/8/4K3 w - - 0 1"
            .parse()
            .expect("valid fen");
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = HangingPieceDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::HangingPiece);
        assert_eq!(tags[0].victims, vec!["d5"]);
        assert_eq!(tags[0].target_square.as_deref(), Some("d5"));
        assert!((tags[0].confidence - 0.95).abs() < f32::EPSILON);
        assert!(tags[0].evidence.defended_by.is_empty());
    }

    #[test]
    fn no_hanging_pieces_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = HangingPieceDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn ignores_defended_pieces() {
        // Black knight on d5 attacked by white bishop on f3, defended by black queen on e6
        // Equal exchange — not hanging
        let board: Board = "4k3/8/4q3/3n4/8/5B2/8/4K3 w - - 0 1"
            .parse()
            .expect("valid fen");
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = HangingPieceDetector.detect(&ctx);
        // Knight has 1 attacker and 1 defender — not hanging
        assert!(
            tags.iter()
                .all(|t| t.target_square.as_deref() != Some("d5")),
            "defended knight should not be flagged as hanging"
        );
    }
}
