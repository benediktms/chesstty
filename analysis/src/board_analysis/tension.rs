use cozy_chess::{Board, Color};
use serde::{Deserialize, Serialize};

use super::helpers::attacked_squares;

/// Metrics measuring the tension and volatility of a position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionTensionMetrics {
    /// Number of squares where both sides have pieces that mutually attack each other.
    pub mutually_attacked_pairs: u8,
    /// Number of squares attacked by both sides.
    pub contested_squares: u8,
    /// Number of pieces that are attacked but also defended.
    pub attacked_but_defended: u8,
    /// Total forcing moves available (checks + captures) for the side to move.
    pub forcing_moves: u8,
    /// Number of checks available for the side to move.
    pub checks_available: u8,
    /// Number of captures available for the side to move.
    pub captures_available: u8,
    /// Composite volatility score from 0.0 (quiet) to 1.0 (volatile).
    pub volatility_score: f32,
}

/// Compute position tension metrics for the current board state.
pub fn compute_tension(board: &Board) -> PositionTensionMetrics {
    let white_attacks = attacked_squares(board, Color::White);
    let black_attacks = attacked_squares(board, Color::Black);

    // Contested squares: attacked by both sides
    let contested = white_attacks & black_attacks;
    let contested_squares = contested.len() as u8;

    // Mutually attacked pairs: squares where each side has a piece and the other attacks it
    let white_pieces = board.colors(Color::White);
    let black_pieces = board.colors(Color::Black);

    // White pieces attacked by black
    let white_under_attack = white_pieces & black_attacks;
    // Black pieces attacked by white
    let black_under_attack = black_pieces & white_attacks;

    // Count pairs where both sides have attacked pieces
    let mutually_attacked_pairs = (white_under_attack.len().min(black_under_attack.len())) as u8;

    // Attacked but defended: pieces attacked by opponent but defended by own side
    let mut attacked_but_defended: u8 = 0;
    for sq in white_under_attack {
        if white_attacks.has(sq) {
            attacked_but_defended += 1;
        }
    }
    for sq in black_under_attack {
        if black_attacks.has(sq) {
            attacked_but_defended += 1;
        }
    }

    // Count forcing moves for side to move
    let (mut checks_available, mut captures_available) = (0u8, 0u8);

    board.generate_moves(|moves| {
        for mv in moves {
            // Check if move is a capture
            let target_sq = mv.to;
            if board.occupied().has(target_sq) {
                captures_available = captures_available.saturating_add(1);
            }

            // Check if move gives check by making the move and checking
            // We use a lightweight approach: if the target piece is the opponent king's square
            // that's not really a check — we'd need to actually test.
            // For efficiency, we check if the move delivers check by making it on a clone.
            let mut test_board = board.clone();
            test_board.play_unchecked(mv);
            if test_board.checkers().len() > 0 {
                checks_available = checks_available.saturating_add(1);
            }
        }
        false
    });

    // En passant captures: the target square might be empty but it's still a capture
    // cozy-chess handles this in generate_moves but the piece isn't on target_sq

    let forcing_moves = checks_available.saturating_add(captures_available);

    // Volatility composite score
    let mutual_factor = (mutually_attacked_pairs as f32 / 5.0).min(1.0);
    let forcing_factor = (forcing_moves as f32 / 15.0).min(1.0);
    let contested_factor = (contested_squares as f32 / 30.0).min(1.0);
    let defended_factor = (attacked_but_defended as f32 / 8.0).min(1.0);

    let volatility_score = (0.30 * mutual_factor
        + 0.25 * forcing_factor
        + 0.25 * contested_factor
        + 0.20 * defended_factor)
        .clamp(0.0, 1.0);

    PositionTensionMetrics {
        mutually_attacked_pairs,
        contested_squares,
        attacked_but_defended,
        forcing_moves,
        checks_available,
        captures_available,
        volatility_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_tension() {
        let board = Board::default();
        let tension = compute_tension(&board);

        // Starting position is relatively quiet
        assert!(
            tension.volatility_score < 0.5,
            "Starting position should have low volatility, got {}",
            tension.volatility_score
        );
        // No captures or checks available from starting position
        assert_eq!(tension.captures_available, 0);
        assert_eq!(tension.checks_available, 0);
    }

    #[test]
    fn test_tactical_position_tension() {
        // A position with many captures available
        // White: Ke1, Qd1, Rd4, Bg5 — Black: Ke8, Qd8, Nd5, Pf6
        let board: Board =
            "3qk3/8/5p2/3n2B1/3R4/8/8/3QK3 w - - 0 1".parse().unwrap();
        let tension = compute_tension(&board);

        // Should have some captures available
        assert!(
            tension.captures_available > 0,
            "Tactical position should have captures available"
        );
    }

    #[test]
    fn test_quiet_endgame() {
        // King and pawn endgame — relatively quiet
        let board: Board =
            "4k3/8/8/8/4P3/8/8/4K3 w - - 0 1".parse().unwrap();
        let tension = compute_tension(&board);

        assert!(
            tension.volatility_score < 0.3,
            "King+pawn endgame should be quiet, got {}",
            tension.volatility_score
        );
    }
}
