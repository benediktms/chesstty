use cozy_chess::Move;
use std::collections::HashMap;

/// A parsed PGN game
#[derive(Debug, Clone)]
pub struct PgnGame {
    pub tags: HashMap<String, String>,
    pub moves: Vec<PgnMove>,
    pub result: GameResult,
}

/// A single move in PGN with metadata
#[derive(Debug, Clone)]
pub struct PgnMove {
    pub mv: Move,
    pub san: String,
    pub comment: Option<String>,
    pub nags: Vec<u8>, // Numeric Annotation Glyphs (!!, ?, etc.)
}

#[derive(Debug, Clone)]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
    Ongoing,
}

/// Parse a PGN string into a game
pub fn parse_pgn(input: &str) -> Result<PgnGame, PgnError> {
    // FUTURE WORK: Implement PGN import - deferred as future enhancement
    todo!("PGN parser not yet implemented")
}

#[derive(Debug, thiserror::Error)]
pub enum PgnError {
    #[error("Invalid PGN format")]
    InvalidFormat,
    #[error("Invalid tag: {0}")]
    InvalidTag(String),
    #[error("SAN parse error: {0}")]
    SanError(#[from] super::san::SanError),
}
