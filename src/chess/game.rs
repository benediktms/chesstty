use cozy_chess::{Board, Color, GameStatus, Move, Piece, Square};
use std::collections::HashMap;

/// Main game state wrapper around cozy-chess Board
#[derive(Debug, Clone)]
pub struct Game {
    position: Board,
    history: Vec<HistoryEntry>,
    pgn_tags: HashMap<String, String>,
    start_position: StartPosition,
}

/// Snapshot of state before a move (for efficient undo)
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub mv: Move,
    pub captured: Option<(Piece, Color)>, // Piece and its color
    pub castling_rights: u8,
    pub en_passant: Option<Square>,
    pub halfmove_clock: u8,
}

/// Starting position of the game
#[derive(Debug, Clone)]
pub enum StartPosition {
    Standard,
    Fen(String),
}

impl Game {
    /// Create a new game from the standard starting position
    pub fn new() -> Self {
        Self {
            position: Board::default(),
            history: Vec::new(),
            pgn_tags: HashMap::new(),
            start_position: StartPosition::Standard,
        }
    }

    /// Create a game from a FEN string
    pub fn from_fen(fen: &str) -> Result<Self, GameError> {
        let position = crate::chess::fen::parse_fen(fen)?;
        Ok(Self {
            position,
            history: Vec::new(),
            pgn_tags: HashMap::new(),
            start_position: StartPosition::Fen(fen.to_string()),
        })
    }

    /// Get the current board position
    pub fn position(&self) -> &Board {
        &self.position
    }

    /// Get the move history
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Make a move on the board
    pub fn make_move(&mut self, mv: Move) -> Result<(), GameError> {
        // Validate move is legal
        if !self.legal_moves().contains(&mv) {
            return Err(GameError::IllegalMove);
        }

        // Snapshot state for undo (simplified - cozy-chess stores this internally)
        let captured = self.position.piece_on(mv.to)
            .and_then(|piece| self.position.color_on(mv.to).map(|color| (piece, color)));

        let entry = HistoryEntry {
            mv,
            captured,
            castling_rights: 0, // TODO: Extract from board
            en_passant: None,   // TODO: Extract from board
            halfmove_clock: 0,  // TODO: Extract from board
        };

        // Play the move
        self.position.play_unchecked(mv);
        self.history.push(entry);

        Ok(())
    }

    /// Undo the last move
    pub fn undo(&mut self) -> Result<(), GameError> {
        if self.history.is_empty() {
            return Err(GameError::NothingToUndo);
        }

        self.history.pop();
        self.rebuild_position()?;

        Ok(())
    }

    /// Get all legal moves for the current position
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        self.position.generate_moves(|mvs| {
            moves.extend(mvs);
            false
        });
        moves
    }

    /// Get the current game status
    pub fn status(&self) -> GameStatus {
        self.position.status()
    }

    /// Get the side to move
    pub fn side_to_move(&self) -> Color {
        self.position.side_to_move()
    }

    /// Export position to FEN string
    pub fn to_fen(&self) -> String {
        crate::chess::fen::format_fen(&self.position)
    }

    /// Rebuild position from start + history (for undo)
    fn rebuild_position(&mut self) -> Result<(), GameError> {
        let mut board = match &self.start_position {
            StartPosition::Standard => Board::default(),
            StartPosition::Fen(fen) => crate::chess::fen::parse_fen(fen)?,
        };

        for entry in &self.history {
            board.play_unchecked(entry.mv);
        }

        self.position = board;
        Ok(())
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("Illegal move")]
    IllegalMove,
    #[error("Nothing to undo")]
    NothingToUndo,
    #[error("FEN parse error: {0}")]
    FenError(#[from] crate::chess::fen::FenError),
}
