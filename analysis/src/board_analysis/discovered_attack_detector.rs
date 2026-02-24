use cozy_chess::Piece;

use super::detector::{TacticalContext, TacticalDetector};
use super::helpers::piece_value;
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects discovered attacks: a piece moves and reveals an attack by a sliding
/// piece behind it onto an enemy piece.
pub struct DiscoveredAttackDetector;

impl TacticalDetector for DiscoveredAttackDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let mv = match ctx.mv {
            Some(m) => m,
            None => return vec![],
        };

        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;
        let mut tags = Vec::new();

        // For each sliding piece of `perspective` on the after board:
        //   - It must NOT be the piece that moved (it's the revealed attacker)
        //   - For each enemy piece it attacks in after_attacks:
        //     - If it did NOT attack that square in before_attacks → discovered attack
        for slider_piece in [Piece::Bishop, Piece::Rook, Piece::Queen] {
            let sliders = ctx.after.pieces(slider_piece) & ctx.after.colors(perspective);
            for slider_sq in sliders {
                // Skip the piece that just moved — it's the mover, not the revealed attacker
                if slider_sq == mv.to {
                    continue;
                }

                // Find enemy pieces in the after position and check if this slider
                // newly attacks them (attacks after but not before).
                // The attack map gives us "who attacks square X from color C", not
                // "which squares does piece P attack". So we iterate enemy pieces.
                let enemy_pieces = ctx.after.colors(enemy);
                for target_sq in enemy_pieces {
                    let Some(_target_piece) = ctx.after.piece_on(target_sq) else {
                        continue;
                    };

                    // Check if slider_sq attacks target_sq in AFTER position
                    let attacks_after = ctx
                        .after_attacks
                        .attackers_of(target_sq, perspective)
                        .iter()
                        .any(|a| a.from == slider_sq);

                    if !attacks_after {
                        continue;
                    }

                    // Check if slider_sq attacked target_sq in BEFORE position
                    let attacks_before = ctx
                        .before_attacks
                        .attackers_of(target_sq, perspective)
                        .iter()
                        .any(|a| a.from == slider_sq);

                    if attacks_before {
                        // Was already attacking — not a discovered attack
                        continue;
                    }

                    // This is a newly revealed attack by slider_sq on target_sq
                    let target_piece = ctx.after.piece_on(target_sq).unwrap();
                    let moving_piece = ctx.before.piece_on(mv.from).unwrap_or(Piece::Pawn);

                    let confidence = if piece_value(target_piece) >= piece_value(Piece::Rook) {
                        0.8
                    } else {
                        0.65
                    };

                    tags.push(TacticalTag {
                        kind: TacticalTagKind::DiscoveredAttack,
                        attacker: Some(slider_sq.to_string()),
                        victims: vec![target_sq.to_string()],
                        target_square: Some(target_sq.to_string()),
                        confidence,
                        note: Some(format!(
                            "discovered attack: {} moves, revealing {} attack on {}",
                            moving_piece, slider_piece, target_sq
                        )),
                        evidence: TacticalEvidence {
                            lines: vec![],
                            threatened_pieces: vec![target_sq.to_string()],
                            defended_by: vec![],
                        },
                    });
                }
            }
        }

        tags
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
            best_line: None,
        }
    }

    #[test]
    fn detects_discovered_attack_bishop_revealed() {
        // Before: White bishop on a1, white knight on c3 blocking the a1-e5 diagonal,
        //         black queen on e5, white king on a8, black king on h8.
        // The diagonal a1-b2-c3-d4-e5 is blocked by the white knight on c3.
        // After: Knight moves from c3 to f5 (off the diagonal), revealing bishop's attack on e5.
        let before: Board = "K6k/8/8/4q3/8/2N5/8/B7 w - - 0 1".parse().unwrap();
        let after: Board = "K6k/8/8/4qN2/8/8/8/B7 b - - 1 1".parse().unwrap();

        let before_attacks = AttackMap::compute(&before);
        let after_attacks = AttackMap::compute(&after);

        let mv = Move {
            from: Square::C3,
            to: Square::F5,
            promotion: None,
        };

        let ctx = make_ctx(
            &before,
            &after,
            &before_attacks,
            &after_attacks,
            Some(mv),
            Color::White,
        );

        let tags = DiscoveredAttackDetector.detect(&ctx);

        // Should find at least one discovered attack by the bishop on e5
        assert!(
            !tags.is_empty(),
            "expected a discovered attack tag, got none"
        );
        let tag = tags.iter().find(|t| {
            t.attacker.as_deref() == Some("a1") && t.target_square.as_deref() == Some("e5")
        });
        assert!(
            tag.is_some(),
            "expected bishop on a1 to newly attack e5, tags: {:?}",
            tags
        );
        assert_eq!(tag.unwrap().kind, TacticalTagKind::DiscoveredAttack);
        // e5 holds black queen (value >= rook) → confidence 0.8
        assert_eq!(tag.unwrap().confidence, 0.8);
    }

    #[test]
    fn no_discovered_attack_without_move() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = make_ctx(&board, &board, &attacks, &attacks, None, Color::White);

        let tags = DiscoveredAttackDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn no_discovered_attack_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);

        // Simulate e2e4 (no sliding piece revealed)
        let mv = Move {
            from: Square::E2,
            to: Square::E4,
            promotion: None,
        };

        let ctx = make_ctx(&board, &board, &attacks, &attacks, Some(mv), Color::White);
        let tags = DiscoveredAttackDetector.detect(&ctx);
        assert!(tags.is_empty());
    }
}
