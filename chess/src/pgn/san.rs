use cozy_chess::{Board, File, Move, Piece, Rank, Square};

/// Parse Standard Algebraic Notation (SAN) move
pub fn parse_san(board: &Board, san: &str) -> Result<Move, SanError> {
    // TODO: Implement SAN parser
    // For now, return error
    Err(SanError::NotImplemented)
}

/// Format a move as SAN
pub fn format_san(board: &Board, mv: Move) -> String {
    // TODO: Implement SAN formatter
    // For now, use UCI format as fallback
    format_move_simple(mv)
}

fn format_move_simple(mv: Move) -> String {
    format!("{}{}", format_square(mv.from), format_square(mv.to))
}

fn format_square(sq: Square) -> String {
    let file = match sq.file() {
        File::A => 'a',
        File::B => 'b',
        File::C => 'c',
        File::D => 'd',
        File::E => 'e',
        File::F => 'f',
        File::G => 'g',
        File::H => 'h',
    };
    let rank = (sq.rank() as u8 + 1).to_string();
    format!("{}{}", file, rank)
}

#[derive(Debug, thiserror::Error)]
pub enum SanError {
    #[error("SAN parser not yet implemented")]
    NotImplemented,
    #[error("No legal move found for: {0}")]
    NoLegalMove(String),
    #[error("Ambiguous move: {0}")]
    AmbiguousMove(String),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Invalid square: {0}")]
    InvalidSquare(String),
    #[error("Invalid file: {0}")]
    InvalidFile(char),
    #[error("Invalid rank: {0}")]
    InvalidRank(char),
    #[error("Invalid promotion: {0}")]
    InvalidPromotion(String),
}
