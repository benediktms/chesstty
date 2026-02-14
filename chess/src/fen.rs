use cozy_chess::{Board, Color, File, Piece, Rank, Square};

/// Parse a FEN string into a Board
pub fn parse_fen(fen: &str) -> Result<Board, FenError> {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.is_empty() {
        return Err(FenError::InvalidFormat);
    }

    // For now, use cozy-chess's built-in FEN parsing if available
    // Otherwise we'll implement custom parsing
    fen.parse().map_err(|_| FenError::InvalidFormat)
}

/// Format a Board as a FEN string
pub fn format_fen(board: &Board) -> String {
    // Use cozy-chess's Display implementation
    board.to_string()
}

fn parse_color(s: &str) -> Result<Color, FenError> {
    match s {
        "w" => Ok(Color::White),
        "b" => Ok(Color::Black),
        _ => Err(FenError::InvalidColor(s.to_string())),
    }
}

fn parse_piece(c: char) -> Result<(Piece, Color), FenError> {
    let piece = match c.to_ascii_lowercase() {
        'p' => Piece::Pawn,
        'n' => Piece::Knight,
        'b' => Piece::Bishop,
        'r' => Piece::Rook,
        'q' => Piece::Queen,
        'k' => Piece::King,
        _ => return Err(FenError::InvalidPiece(c)),
    };

    let color = if c.is_uppercase() {
        Color::White
    } else {
        Color::Black
    };

    Ok((piece, color))
}

#[derive(Debug, thiserror::Error)]
pub enum FenError {
    #[error("Invalid FEN format")]
    InvalidFormat,
    #[error("Invalid board layout")]
    InvalidBoardLayout,
    #[error("Invalid color: {0}")]
    InvalidColor(String),
    #[error("Invalid piece: {0}")]
    InvalidPiece(char),
}
