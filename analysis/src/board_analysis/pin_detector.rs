use cozy_chess::Piece;

use super::attack_map::PinInfo;
use super::detector::{TacticalContext, TacticalDetector};
use super::tactical_types::{TacticalEvidence, TacticalLine, TacticalTag, TacticalTagKind};

/// Detects pins: a sliding piece restricting an enemy piece's movement
/// because moving it would expose a higher-value piece behind it.
pub struct PinDetector;

impl TacticalDetector for PinDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;

        ctx.after_attacks
            .pins()
            .iter()
            .filter(|pin| {
                // Keep pins where the pinner belongs to the side that just moved.
                ctx.after.colors(perspective).has(pin.pinner.from)
            })
            .map(|pin| pin_to_tag(ctx, pin))
            .collect()
    }
}

fn pin_to_tag(ctx: &TacticalContext, pin: &PinInfo) -> TacticalTag {
    let pinned_to_piece = ctx.after.piece_on(pin.pinned_to);
    let is_absolute = pinned_to_piece == Some(Piece::King);

    // Build the through-squares on the ray between pinner and pinned_to,
    // excluding endpoints.
    let through: Vec<String> = {
        let mut squares = Vec::new();
        for sq in pin.ray {
            if sq != pin.pinner.from && sq != pin.pinned_to && sq != pin.pinned {
                squares.push(sq.to_string());
            }
        }
        squares
    };

    TacticalTag {
        kind: TacticalTagKind::Pin,
        attacker: Some(pin.pinner.from.to_string()),
        victims: vec![pin.pinned.to_string()],
        target_square: Some(pin.pinned_to.to_string()),
        confidence: if is_absolute { 1.0 } else { 0.8 },
        note: Some(if is_absolute {
            format!(
                "absolute pin: {} pins {} to king",
                pin.pinner.piece, pin.pinned
            )
        } else {
            format!(
                "relative pin: {} pins {} to {}",
                pin.pinner.piece, pin.pinned, pin.pinned_to
            )
        }),
        evidence: TacticalEvidence {
            lines: vec![TacticalLine {
                from: pin.pinner.from.to_string(),
                through,
                to: pin.pinned_to.to_string(),
            }],
            threatened_pieces: vec![pin.pinned.to_string()],
            defended_by: vec![],
        },
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
    fn detects_absolute_pin_bishop_to_king() {
        // White bishop on a4 pins black knight on c6 to black king on e8
        let board: Board = "4k3/8/2n5/8/B7/8/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = PinDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::Pin);
        assert_eq!(tags[0].attacker.as_deref(), Some("a4"));
        assert_eq!(tags[0].victims, vec!["c6"]);
        assert_eq!(tags[0].target_square.as_deref(), Some("e8"));
        assert_eq!(tags[0].confidence, 1.0);
        assert!(tags[0].note.as_ref().unwrap().contains("absolute pin"));
    }

    #[test]
    fn detects_relative_pin_rook_to_queen() {
        // White rook on a1 pins black knight on a5 to black queen on a8
        let board: Board = "q3k3/8/8/n7/8/8/8/R3K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = PinDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::Pin);
        assert_eq!(tags[0].confidence, 0.8);
        assert!(tags[0].note.as_ref().unwrap().contains("relative pin"));
    }

    #[test]
    fn no_pins_in_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = PinDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn ignores_pins_by_opponent() {
        // Black bishop on a4 pins white knight on c2 to white king on d1...
        // but we ask for White's perspective — should find nothing.
        // (Note: side_to_move_before=Black would find it)
        let board: Board = "4k3/8/8/8/b7/8/2N5/3K4 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);

        // White perspective: no pins by White
        let ctx_white = ctx_from_after(&board, &attacks, Color::White);
        assert!(PinDetector.detect(&ctx_white).is_empty());

        // Black perspective: finds the pin
        let ctx_black = ctx_from_after(&board, &attacks, Color::Black);
        let tags = PinDetector.detect(&ctx_black);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].attacker.as_deref(), Some("a4"));
    }

    #[test]
    fn evidence_line_has_correct_endpoints() {
        let board: Board = "4k3/8/2n5/8/B7/8/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = PinDetector.detect(&ctx);
        let line = &tags[0].evidence.lines[0];

        assert_eq!(line.from, "a4");
        assert_eq!(line.to, "e8");
    }
}
