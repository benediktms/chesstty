use cozy_chess::Board;

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

#[derive(Debug, thiserror::Error)]
pub enum FenError {
    #[error("Invalid FEN format")]
    InvalidFormat,
    #[error("Invalid board layout")]
    InvalidBoardLayout,
}
