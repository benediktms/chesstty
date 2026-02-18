use cozy_chess::{BitBoard, Board, Color, File, Piece, Square};

/// Returns the union of all attack squares for a given color.
pub fn attacked_squares(board: &Board, color: Color) -> BitBoard {
    let mut attacks = BitBoard::EMPTY;
    let occupied = board.occupied();

    for piece in Piece::ALL {
        let pieces = board.pieces(piece) & board.colors(color);
        for sq in pieces {
            attacks |= piece_attacks(board, sq, piece, color);
        }
    }

    // We don't want to include the occupied squares of our own pieces
    // in the "attacks" set — attacks means squares the pieces control.
    let _ = occupied; // occupied is used implicitly via piece_attacks
    attacks
}

/// Returns all pieces of `color` that attack the given square.
pub fn attackers_of(board: &Board, sq: Square, color: Color) -> BitBoard {
    let occupied = board.occupied();
    let color_pieces = board.colors(color);

    let mut attackers = BitBoard::EMPTY;

    // Pawn attacks: a pawn of `color` attacks `sq` if `sq` is in the pawn's attack set.
    // Equivalently, we look from `sq` as if it were the opposite color's pawn.
    let pawn_attacks = cozy_chess::get_pawn_attacks(sq, !color);
    attackers |= pawn_attacks & board.pieces(Piece::Pawn) & color_pieces;

    // Knight attacks
    let knight_attacks = cozy_chess::get_knight_moves(sq);
    attackers |= knight_attacks & board.pieces(Piece::Knight) & color_pieces;

    // Bishop/Queen (diagonal)
    let bishop_attacks = cozy_chess::get_bishop_moves(sq, occupied);
    attackers |= bishop_attacks & (board.pieces(Piece::Bishop) | board.pieces(Piece::Queen)) & color_pieces;

    // Rook/Queen (orthogonal)
    let rook_attacks = cozy_chess::get_rook_moves(sq, occupied);
    attackers |= rook_attacks & (board.pieces(Piece::Rook) | board.pieces(Piece::Queen)) & color_pieces;

    // King attacks
    let king_attacks = cozy_chess::get_king_moves(sq);
    attackers |= king_attacks & board.pieces(Piece::King) & color_pieces;

    attackers
}

/// Standard piece values in centipawns.
pub fn piece_value(piece: Piece) -> u16 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20000,
    }
}

/// Returns the attack bitboard for a specific piece on a square.
pub fn piece_attacks(board: &Board, sq: Square, piece: Piece, color: Color) -> BitBoard {
    let occupied = board.occupied();
    match piece {
        Piece::Pawn => cozy_chess::get_pawn_attacks(sq, color),
        Piece::Knight => cozy_chess::get_knight_moves(sq),
        Piece::Bishop => cozy_chess::get_bishop_moves(sq, occupied),
        Piece::Rook => cozy_chess::get_rook_moves(sq, occupied),
        Piece::Queen => {
            cozy_chess::get_bishop_moves(sq, occupied) | cozy_chess::get_rook_moves(sq, occupied)
        }
        Piece::King => cozy_chess::get_king_moves(sq),
    }
}

/// Returns the files adjacent to the king (including the king's own file), clamped to the board.
pub fn king_zone_files(king_sq: Square) -> impl Iterator<Item = File> {
    let king_file = king_sq.file() as i8;
    let min_file = (king_file - 1).max(0) as u8;
    let max_file = (king_file + 1).min(7) as u8;
    (min_file..=max_file).filter_map(|f| File::try_index(f as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_values() {
        assert_eq!(piece_value(Piece::Pawn), 100);
        assert_eq!(piece_value(Piece::Knight), 320);
        assert_eq!(piece_value(Piece::Bishop), 330);
        assert_eq!(piece_value(Piece::Rook), 500);
        assert_eq!(piece_value(Piece::Queen), 900);
        assert_eq!(piece_value(Piece::King), 20000);
    }

    #[test]
    fn test_attacked_squares_starting_position() {
        let board: Board = Board::default();
        let white_attacks = attacked_squares(&board, Color::White);
        // White pawns attack ranks 3 squares, knights attack some squares
        assert!(white_attacks.len() > 0);
    }

    #[test]
    fn test_attackers_of_center() {
        let board: Board = Board::default();
        // e4 square — no white pieces attack it directly from starting position
        // except the pawn on d2 and f2 don't attack e4 (they attack d3/f3 and e3/g3)
        // Actually d2 pawn attacks e3, not e4. Let's check a simpler case.
        // In starting position, e3 is attacked by the d2 and f2 pawns
        let e3 = Square::E3;
        let white_attackers = attackers_of(&board, e3, Color::White);
        // d2 pawn attacks e3, f2 pawn attacks e3
        assert!(white_attackers.len() >= 2);
    }

    #[test]
    fn test_piece_attacks_knight() {
        let board: Board = Board::default();
        let g1 = Square::G1;
        let attacks = piece_attacks(&board, g1, Piece::Knight, Color::White);
        // Knight on g1 attacks f3 and h3
        assert!(attacks.has(Square::F3));
        assert!(attacks.has(Square::H3));
    }
}
