//! UCI (Universal Chess Interface) utilities

use cozy_chess::{File, Move, Rank, Square};

use crate::converters::{format_piece, format_square};

/// Convert UCI castling notation to cozy_chess notation
///
/// UCI uses standard notation (king moves 2 squares): e1g1, e1c1, e8g8, e8c8
/// cozy_chess uses king-to-rook notation: e1h1, e1a1, e8h8, e8a8
///
/// This function checks if the move is a castling move and converts it to the
/// appropriate cozy_chess format by finding the matching legal move.
pub fn convert_uci_castling_to_cozy(mv: Move, legal_moves: &[Move]) -> Move {
    // Check if this looks like a UCI castling move (king moving 2 squares on rank 1 or 8)
    let is_rank_1_or_8 = matches!(mv.from.rank(), Rank::First | Rank::Eighth);
    let is_e_file = matches!(mv.from.file(), File::E);
    let is_g_or_c_file = matches!(mv.to.file(), File::G | File::C);

    if is_rank_1_or_8 && is_e_file && is_g_or_c_file && mv.promotion.is_none() {
        // This looks like a castling move in UCI notation
        // Convert to cozy_chess notation
        let target_square = match (mv.from.rank(), mv.to.file()) {
            (Rank::First, File::G) => Square::new(File::H, Rank::First), // e1g1 → e1h1 (white kingside)
            (Rank::First, File::C) => Square::new(File::A, Rank::First), // e1c1 → e1a1 (white queenside)
            (Rank::Eighth, File::G) => Square::new(File::H, Rank::Eighth), // e8g8 → e8h8 (black kingside)
            (Rank::Eighth, File::C) => Square::new(File::A, Rank::Eighth), // e8c8 → e8a8 (black queenside)
            _ => return mv,                                                // Not a castling move
        };

        let converted = Move {
            from: mv.from,
            to: target_square,
            promotion: None,
        };

        // Verify the converted move is in the legal moves list
        if legal_moves.contains(&converted) {
            return converted;
        }
    }

    // Not a castling move or conversion didn't work, return original
    mv
}

/// Format a move in UCI notation (e.g., "e2e4", "e7e8q")
pub fn format_uci_move(mv: Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(format_piece(promo));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::Piece;

    #[test]
    fn test_format_uci_move() {
        let mv = Move {
            from: Square::new(File::E, Rank::Second),
            to: Square::new(File::E, Rank::Fourth),
            promotion: None,
        };
        assert_eq!(format_uci_move(mv), "e2e4");
    }

    #[test]
    fn test_format_uci_move_with_promotion() {
        let mv = Move {
            from: Square::new(File::E, Rank::Seventh),
            to: Square::new(File::E, Rank::Eighth),
            promotion: Some(Piece::Queen),
        };
        assert_eq!(format_uci_move(mv), "e7e8q");
    }
}
