use cozy_chess::{File, Piece, Rank, Square};

use super::detector::{TacticalContext, TacticalDetector};
use super::helpers::{piece_attacks, piece_value};
use super::tactical_types::{TacticalEvidence, TacticalLine, TacticalTag, TacticalTagKind};

/// Detects skewers: a sliding piece attacks a high-value enemy piece, and behind it
/// along the same ray is a lower-value enemy piece. The front piece is forced to move,
/// exposing the piece behind it.
pub struct SkewerDetector;

impl TacticalDetector for SkewerDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;
        let board = ctx.after;

        let mut tags = Vec::new();

        for slider_piece in [Piece::Bishop, Piece::Rook, Piece::Queen] {
            let sliders = board.pieces(slider_piece) & board.colors(perspective);

            for slider_sq in sliders {
                let attacks = piece_attacks(board, slider_sq, slider_piece, perspective);
                let enemy_pieces = board.colors(enemy);

                for front_sq in attacks & enemy_pieces {
                    let Some(front_piece) = board.piece_on(front_sq) else {
                        continue;
                    };

                    let Some(back_sq) = find_piece_behind(board, slider_sq, front_sq, enemy)
                    else {
                        continue;
                    };

                    let Some(back_piece) = board.piece_on(back_sq) else {
                        continue;
                    };

                    // Skewer: front piece must be higher value than back piece
                    // (opposite of a pin where back_val > front_val)
                    let is_skewer = piece_value(front_piece) > piece_value(back_piece)
                        || front_piece == Piece::King;
                    if !is_skewer {
                        continue;
                    }

                    let is_king_skewer = front_piece == Piece::King;

                    let through = ray_through_squares(slider_sq, front_sq, back_sq);

                    tags.push(TacticalTag {
                        kind: TacticalTagKind::Skewer,
                        attacker: Some(slider_sq.to_string()),
                        victims: vec![front_sq.to_string(), back_sq.to_string()],
                        target_square: Some(back_sq.to_string()),
                        confidence: if is_king_skewer { 0.9 } else { 0.75 },
                        note: Some(format!(
                            "skewer: {} attacks {} through to {}",
                            slider_piece, front_sq, back_sq
                        )),
                        evidence: TacticalEvidence {
                            lines: vec![TacticalLine {
                                from: slider_sq.to_string(),
                                through,
                                to: back_sq.to_string(),
                            }],
                            threatened_pieces: vec![
                                front_sq.to_string(),
                                back_sq.to_string(),
                            ],
                            defended_by: vec![],
                        },
                    });
                }
            }
        }

        tags
    }
}

/// Walks along the ray from `slider_sq` through `front_sq` and returns the first
/// enemy-colored piece found behind `front_sq`, if any.
fn find_piece_behind(
    board: &cozy_chess::Board,
    slider_sq: Square,
    front_sq: Square,
    target_color: cozy_chess::Color,
) -> Option<Square> {
    let slider_rank = slider_sq.rank() as i8;
    let slider_file = slider_sq.file() as i8;
    let front_rank = front_sq.rank() as i8;
    let front_file = front_sq.file() as i8;

    let dr = (front_rank - slider_rank).signum();
    let df = (front_file - slider_file).signum();

    if dr == 0 && df == 0 {
        return None;
    }

    let mut r = front_rank + dr;
    let mut f = front_file + df;

    while (0..8).contains(&r) && (0..8).contains(&f) {
        let rank = Rank::try_index(r as usize)?;
        let file = File::try_index(f as usize)?;
        let sq = Square::new(file, rank);

        if board.occupied().has(sq) {
            if board.colors(target_color).has(sq) {
                return Some(sq);
            }
            return None;
        }

        r += dr;
        f += df;
    }

    None
}

/// Returns the intermediate squares on the ray from `slider_sq` to `back_sq`,
/// excluding both endpoints and the `front_sq`.
fn ray_through_squares(slider_sq: Square, front_sq: Square, back_sq: Square) -> Vec<String> {
    let slider_rank = slider_sq.rank() as i8;
    let slider_file = slider_sq.file() as i8;
    let back_rank = back_sq.rank() as i8;
    let back_file = back_sq.file() as i8;

    let dr = (back_rank - slider_rank).signum();
    let df = (back_file - slider_file).signum();

    let mut through = Vec::new();
    let mut r = slider_rank + dr;
    let mut f = slider_file + df;

    while (0..8).contains(&r) && (0..8).contains(&f) {
        let rank = match Rank::try_index(r as usize) {
            Some(rk) => rk,
            None => break,
        };
        let file = match File::try_index(f as usize) {
            Some(fl) => fl,
            None => break,
        };
        let sq = Square::new(file, rank);

        if sq == back_sq {
            break;
        }

        // Include intermediate squares (between slider and back, excluding endpoints and front)
        if sq != slider_sq && sq != front_sq {
            through.push(sq.to_string());
        }

        r += dr;
        f += df;
    }

    through
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
    fn detects_rook_skewer_queen_through_to_bishop() {
        // White rook on a1 skewers black queen on a5 (high value, forced to move),
        // with black bishop on a8 behind it (lower value).
        // FEN: bishop on a8, queen on a5, rook on a1, kings elsewhere
        let board: Board = "b3k3/8/8/q7/8/8/8/R3K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = SkewerDetector.detect(&ctx);

        assert!(!tags.is_empty(), "expected a skewer to be detected");
        let skewer = tags
            .iter()
            .find(|t| t.attacker.as_deref() == Some("a1"))
            .expect("expected skewer from a1");
        assert_eq!(skewer.kind, TacticalTagKind::Skewer);
        assert!(skewer.victims.contains(&"a5".to_string()), "front piece (queen) should be a victim");
        assert!(skewer.victims.contains(&"a8".to_string()), "back piece (bishop) should be a victim");
        assert_eq!(skewer.target_square.as_deref(), Some("a8"));
        assert_eq!(skewer.confidence, 0.75);
        assert!(skewer.note.as_ref().unwrap().contains("skewer"));
    }

    #[test]
    fn detects_bishop_skewer_king() {
        // White bishop on a2 skewers black king on c4 (front, forced to move),
        // with black rook on e6 behind it (lower value than king).
        // Diagonal a2-c4-e6; white king on h1, black king on c4.
        // It's black's turn (black king is in check from the bishop — valid).
        // We pass Color::White as perspective so we detect White's skewers.
        let board: Board = "8/8/4r3/8/2k5/8/B7/7K b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = SkewerDetector.detect(&ctx);

        let king_skewer = tags
            .iter()
            .find(|t| t.kind == TacticalTagKind::Skewer && t.victims.contains(&"c4".to_string()))
            .expect("expected bishop to skewer the king on c4");

        assert_eq!(king_skewer.attacker.as_deref(), Some("a2"));
        assert_eq!(king_skewer.confidence, 0.9, "king skewer should have 0.9 confidence");
        assert!(king_skewer.victims.contains(&"e6".to_string()), "rook behind king should be a victim");
    }

    #[test]
    fn no_skewers_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = SkewerDetector.detect(&ctx);
        assert!(tags.is_empty(), "no skewers should be detected from the starting position");
    }

    #[test]
    fn ignores_skewers_by_opponent() {
        // Black rook on h8 skewers white queen on h4 (front, higher value)
        // with white bishop on h1 (back, lower value).
        // White king on a1, black king on f6 (not on the h-file or bishop diagonal).
        // It's black's turn; white queen on h4 is attacked by black rook (check), valid.
        let board: Board = "7r/8/5k2/8/7Q/8/8/K6B b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);

        // White perspective: White queen on h4 doesn't skewer anything (rook on h8 is lower
        // value than queen, so white queen → rook → nothing is not a skewer for white).
        let ctx_white = ctx_from_after(&board, &attacks, Color::White);
        let white_tags = SkewerDetector.detect(&ctx_white);
        assert!(
            white_tags.is_empty(),
            "White perspective should find no skewers in this position, found: {:?}",
            white_tags
        );

        // Black perspective: Black rook on h8 skewers white queen on h4 (front, higher value)
        // with white bishop on h1 (back, lower value).
        let ctx_black = ctx_from_after(&board, &attacks, Color::Black);
        let black_tags = SkewerDetector.detect(&ctx_black);
        assert!(
            !black_tags.is_empty(),
            "Black perspective should detect the rook skewer"
        );
        let skewer = black_tags
            .iter()
            .find(|t| t.attacker.as_deref() == Some("h8"))
            .expect("expected skewer from h8");
        assert!(skewer.victims.contains(&"h4".to_string()), "queen on h4 should be front victim");
        assert!(skewer.victims.contains(&"h1".to_string()), "bishop on h1 should be back victim");
    }
}
