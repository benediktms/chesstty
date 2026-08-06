use cozy_chess::{Color as ChessColor, Piece};

/// Maps a chess piece + color to a Unicode symbol.
pub fn piece_to_unicode(piece: Piece, color: ChessColor) -> &'static str {
    match (piece, color) {
        (Piece::King, ChessColor::White) => "\u{2654}",   // ♔
        (Piece::Queen, ChessColor::White) => "\u{2655}",  // ♕
        (Piece::Rook, ChessColor::White) => "\u{2656}",   // ♖
        (Piece::Bishop, ChessColor::White) => "\u{2657}", // ♗
        (Piece::Knight, ChessColor::White) => "\u{2658}", // ♘
        (Piece::Pawn, ChessColor::White) => "\u{2659}",   // ♙
        (Piece::King, ChessColor::Black) => "\u{265a}",   // ♚
        (Piece::Queen, ChessColor::Black) => "\u{265b}",  // ♛
        (Piece::Rook, ChessColor::Black) => "\u{265c}",   // ♜
        (Piece::Bishop, ChessColor::Black) => "\u{265d}", // ♝
        (Piece::Knight, ChessColor::Black) => "\u{265e}", // ♞
        (Piece::Pawn, ChessColor::Black) => "\u{265f}",   // ♟
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_pieces_to_unicode() {
        assert_eq!(piece_to_unicode(Piece::King, ChessColor::White), "\u{2654}");
        assert_eq!(
            piece_to_unicode(Piece::Queen, ChessColor::White),
            "\u{2655}"
        );
        assert_eq!(piece_to_unicode(Piece::Rook, ChessColor::White), "\u{2656}");
        assert_eq!(
            piece_to_unicode(Piece::Bishop, ChessColor::White),
            "\u{2657}"
        );
        assert_eq!(
            piece_to_unicode(Piece::Knight, ChessColor::White),
            "\u{2658}"
        );
        assert_eq!(piece_to_unicode(Piece::Pawn, ChessColor::White), "\u{2659}");
    }

    #[test]
    fn test_black_pieces_to_unicode() {
        assert_eq!(piece_to_unicode(Piece::King, ChessColor::Black), "\u{265a}");
        assert_eq!(
            piece_to_unicode(Piece::Queen, ChessColor::Black),
            "\u{265b}"
        );
        assert_eq!(piece_to_unicode(Piece::Rook, ChessColor::Black), "\u{265c}");
        assert_eq!(
            piece_to_unicode(Piece::Bishop, ChessColor::Black),
            "\u{265d}"
        );
        assert_eq!(
            piece_to_unicode(Piece::Knight, ChessColor::Black),
            "\u{265e}"
        );
        assert_eq!(piece_to_unicode(Piece::Pawn, ChessColor::Black), "\u{265f}");
    }
}
