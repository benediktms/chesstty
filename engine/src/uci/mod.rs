pub mod parser;

pub use parser::{format_uci_move, parse_uci_message, parse_uci_move, UciMessage};

#[derive(Debug, thiserror::Error)]
pub enum UciError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Engine has no stdin")]
    NoStdin,
    #[error("Engine has no stdout")]
    NoStdout,
    #[error("Malformed UCI message: {0}")]
    MalformedMessage(String),
    #[error("Unknown UCI message: {0}")]
    UnknownMessage(String),
    #[error("Invalid move: {0}")]
    InvalidMove(String),
    #[error("Invalid square: {0}")]
    InvalidSquare(String),
    #[error("Invalid promotion: {0}")]
    InvalidPromotion(String),
}
