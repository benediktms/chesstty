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
    pub from: Square,
    pub to: Square,
    pub piece: Piece,             // Piece that made the move
    pub piece_color: Color,       // Color of the piece that moved
    pub captured: Option<Piece>,  // Captured piece (simplified)
    pub promotion: Option<Piece>, // Promotion piece if any
    pub san: String,              // Standard Algebraic Notation
    pub fen: String,              // FEN after this move
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
        let position = crate::fen::parse_fen(fen)?;
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
    pub fn make_move(&mut self, mv: Move) -> Result<HistoryEntry, GameError> {
        // Validate move is legal
        if !self.legal_moves().contains(&mv) {
            return Err(GameError::IllegalMove);
        }

        // Snapshot state for undo (simplified - cozy-chess stores this internally)
        let captured = self.position.piece_on(mv.to);

        // Get the piece and color that's making the move
        let piece = self
            .position
            .piece_on(mv.from)
            .ok_or(GameError::IllegalMove)?;
        let piece_color = self
            .position
            .color_on(mv.from)
            .ok_or(GameError::IllegalMove)?;

        // Generate SAN notation before making the move
        let san = generate_san(&self.position, mv, piece);

        // Clone the board and play the move
        // Note: cozy_chess uses immutable boards, so play() returns a new board
        let mut new_position = self.position.clone();
        new_position.play_unchecked(mv);
        self.position = new_position;

        // Get FEN after the move
        let fen = self.to_fen();

        let entry = HistoryEntry {
            mv,
            from: mv.from,
            to: mv.to,
            piece,
            piece_color,
            captured,
            promotion: mv.promotion,
            san,
            fen,
            castling_rights: 0, // TODO: Extract from board
            en_passant: None,   // TODO: Extract from board
            halfmove_clock: 0,  // TODO: Extract from board
        };

        self.history.push(entry.clone());

        Ok(entry)
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
        crate::fen::format_fen(&self.position)
    }

    /// Redo is not implemented (would require storing redo stack)
    pub fn redo(&mut self) -> Result<HistoryEntry, GameError> {
        Err(GameError::NotImplemented)
    }

    /// Rebuild position from start + history (for undo)
    fn rebuild_position(&mut self) -> Result<(), GameError> {
        let mut board = match &self.start_position {
            StartPosition::Standard => Board::default(),
            StartPosition::Fen(fen) => crate::fen::parse_fen(fen)?,
        };

        for entry in &self.history {
            board = board.try_play(entry.mv)
                .map_err(|_| GameError::IllegalMove)?;
        }

        self.position = board;
        Ok(())
    }
}

/// Generate simplified SAN notation for a move
fn generate_san(board: &Board, mv: Move, piece: Piece) -> String {
    let mut san = String::new();

    // Piece prefix (except pawns)
    match piece {
        Piece::King => san.push('K'),
        Piece::Queen => san.push('Q'),
        Piece::Rook => san.push('R'),
        Piece::Bishop => san.push('B'),
        Piece::Knight => san.push('N'),
        Piece::Pawn => {
            // Pawn captures include the file
            if board.piece_on(mv.to).is_some() {
                san.push(file_to_char(mv.from));
            }
        }
    }

    // Capture indicator
    if board.piece_on(mv.to).is_some() {
        san.push('x');
    }

    // Destination square
    san.push(file_to_char(mv.to));
    san.push(rank_to_char(mv.to));

    // Promotion
    if let Some(promo) = mv.promotion {
        san.push('=');
        san.push(match promo {
            Piece::Queen => 'Q',
            Piece::Rook => 'R',
            Piece::Bishop => 'B',
            Piece::Knight => 'N',
            _ => '?',
        });
    }

    san
}

fn file_to_char(square: Square) -> char {
    match square.file() {
        cozy_chess::File::A => 'a',
        cozy_chess::File::B => 'b',
        cozy_chess::File::C => 'c',
        cozy_chess::File::D => 'd',
        cozy_chess::File::E => 'e',
        cozy_chess::File::F => 'f',
        cozy_chess::File::G => 'g',
        cozy_chess::File::H => 'h',
    }
}

fn rank_to_char(square: Square) -> char {
    match square.rank() {
        cozy_chess::Rank::First => '1',
        cozy_chess::Rank::Second => '2',
        cozy_chess::Rank::Third => '3',
        cozy_chess::Rank::Fourth => '4',
        cozy_chess::Rank::Fifth => '5',
        cozy_chess::Rank::Sixth => '6',
        cozy_chess::Rank::Seventh => '7',
        cozy_chess::Rank::Eighth => '8',
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
    #[error("Not implemented")]
    NotImplemented,
    #[error("FEN parse error: {0}")]
    FenError(#[from] crate::fen::FenError),
}
