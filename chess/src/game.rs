use crate::converters::{format_file, format_piece_upper, format_rank};
use cozy_chess::{Board, Color, GameStatus, Move, Piece, Square};

/// Main game state wrapper around cozy-chess Board
#[derive(Debug, Clone)]
pub struct Game {
    position: Board,
    history: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>, // Stack for redo operations
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
    pub board_before: Board,      // Board state before this move (for O(1) undo)
}

/// High-level game phase state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    Setup,
    Playing { turn: Color },
    Paused { resume_turn: Color },
    Ended { result: GameResult, reason: String },
    Analyzing,
}

/// Outcome of a finished game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
}

impl GamePhase {
    /// Derive the current phase from game state.
    pub fn from_game(game: &Game) -> Self {
        match game.status() {
            GameStatus::Ongoing => GamePhase::Playing {
                turn: game.side_to_move(),
            },
            GameStatus::Won => {
                let result = if game.side_to_move() == Color::White {
                    GameResult::BlackWins
                } else {
                    GameResult::WhiteWins
                };
                GamePhase::Ended {
                    result,
                    reason: "Checkmate".to_string(),
                }
            }
            GameStatus::Drawn => GamePhase::Ended {
                result: GameResult::Draw,
                reason: "Draw".to_string(),
            },
        }
    }
}

/// Determines who controls each side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameMode {
    HumanVsHuman,
    HumanVsEngine { human_side: PlayerSide },
    EngineVsEngine,
    Analysis,
    Review,
}

/// Which side a player is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSide {
    White,
    Black,
}

impl From<Color> for PlayerSide {
    fn from(c: Color) -> Self {
        match c {
            Color::White => PlayerSide::White,
            Color::Black => PlayerSide::Black,
        }
    }
}

impl From<PlayerSide> for Color {
    fn from(s: PlayerSide) -> Self {
        match s {
            PlayerSide::White => Color::White,
            PlayerSide::Black => Color::Black,
        }
    }
}

impl Game {
    /// Create a new game from the standard starting position
    pub fn new() -> Self {
        Self {
            position: Board::default(),
            history: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Create a game from a FEN string
    pub fn from_fen(fen: &str) -> Result<Self, GameError> {
        let position = crate::fen::parse_fen(fen)?;
        Ok(Self {
            position,
            history: Vec::new(),
            redo_stack: Vec::new(),
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

        // Clear redo stack on new move (standard undo/redo behavior)
        self.redo_stack.clear();

        // Clone the board before the move for O(1) undo
        let board_before = self.position.clone();

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

        // Play the move (modifies board in place)
        self.position.play(mv);

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
            board_before,
        };

        self.history.push(entry.clone());

        Ok(entry)
    }

    /// Undo the last move - O(1) operation using board snapshots
    pub fn undo(&mut self) -> Result<(), GameError> {
        if self.history.is_empty() {
            return Err(GameError::NothingToUndo);
        }

        // Pop the last move from history
        let entry = self.history.pop().unwrap();

        // Restore board from snapshot (clone to avoid partial move)
        self.position = entry.board_before.clone();

        // Move entry to redo stack
        self.redo_stack.push(entry);

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

    /// Redo a previously undone move - O(1) operation
    pub fn redo(&mut self) -> Result<HistoryEntry, GameError> {
        if self.redo_stack.is_empty() {
            return Err(GameError::NothingToRedo);
        }

        // Pop from redo stack
        let entry = self.redo_stack.pop().unwrap();

        // Apply the move
        self.position.play(entry.mv);

        // Push back to history
        self.history.push(entry.clone());

        Ok(entry)
    }
}

/// Format a move as SAN given a board position.
/// Returns the UCI format as fallback if the piece can't be determined.
pub fn format_move_as_san(board: &Board, mv: Move) -> String {
    // Handle castling
    if let Some(piece) = board.piece_on(mv.from) {
        if piece == Piece::King {
            let from_file = mv.from.file();
            let to_file = mv.to.file();
            if from_file == cozy_chess::File::E
                && (to_file == cozy_chess::File::G || to_file == cozy_chess::File::H)
            {
                return "O-O".to_string();
            }
            if from_file == cozy_chess::File::E
                && (to_file == cozy_chess::File::C || to_file == cozy_chess::File::A)
            {
                return "O-O-O".to_string();
            }
        }
        generate_san(board, mv, piece)
    } else {
        crate::format_uci_move(mv)
    }
}

/// Generate simplified SAN notation for a move
fn generate_san(board: &Board, mv: Move, piece: Piece) -> String {
    let mut san = String::new();

    // Piece prefix (except pawns)
    match piece {
        Piece::Pawn => {
            // Pawn captures include the file
            if board.piece_on(mv.to).is_some() {
                san.push(format_file(mv.from.file()));
            }
        }
        _ => san.push(format_piece_upper(piece)),
    }

    // Capture indicator
    if board.piece_on(mv.to).is_some() {
        san.push('x');
    }

    // Destination square
    san.push(format_file(mv.to.file()));
    san.push(format_rank(mv.to.rank()));

    // Promotion
    if let Some(promo) = mv.promotion {
        san.push('=');
        san.push(format_piece_upper(promo));
    }

    san
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::{File, Move, Piece, Rank, Square};

    fn mv(from_file: File, from_rank: Rank, to_file: File, to_rank: Rank) -> Move {
        Move {
            from: Square::new(from_file, from_rank),
            to: Square::new(to_file, to_rank),
            promotion: None,
        }
    }

    #[test]
    fn test_san_pawn_push() {
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        // e2e4
        let san = format_move_as_san(&board, mv(File::E, Rank::Second, File::E, Rank::Fourth));
        assert_eq!(san, "e4");
    }

    #[test]
    fn test_san_knight_move() {
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        // g1f3
        let san = format_move_as_san(&board, mv(File::G, Rank::First, File::F, Rank::Third));
        assert_eq!(san, "Nf3");
    }

    #[test]
    fn test_san_pawn_capture() {
        // Position where white pawn on e4 can capture black pawn on d5
        let board: Board = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2"
            .parse()
            .unwrap();
        let san = format_move_as_san(&board, mv(File::E, Rank::Fourth, File::D, Rank::Fifth));
        assert_eq!(san, "exd5");
    }

    #[test]
    fn test_san_piece_capture() {
        // Position where white bishop on c4 captures pawn on f7
        let board: Board = "rnbqkbnr/pppppppp/8/8/2B5/8/PPPPPPPP/RNBQK1NR w KQkq - 0 1"
            .parse()
            .unwrap();
        let san = format_move_as_san(&board, mv(File::C, Rank::Fourth, File::F, Rank::Seventh));
        assert_eq!(san, "Bxf7");
    }

    #[test]
    fn test_san_kingside_castle() {
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQK2R w KQkq - 0 1"
            .parse()
            .unwrap();
        // cozy-chess represents castling as king to rook square (e1h1)
        let san = format_move_as_san(&board, mv(File::E, Rank::First, File::H, Rank::First));
        assert_eq!(san, "O-O");
    }

    #[test]
    fn test_san_queenside_castle() {
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R3KBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        // cozy-chess: e1a1
        let san = format_move_as_san(&board, mv(File::E, Rank::First, File::A, Rank::First));
        assert_eq!(san, "O-O-O");
    }

    #[test]
    fn test_san_promotion() {
        let board: Board = "8/P7/8/8/8/8/8/4K2k w - - 0 1".parse().unwrap();
        let m = Move {
            from: Square::new(File::A, Rank::Seventh),
            to: Square::new(File::A, Rank::Eighth),
            promotion: Some(Piece::Queen),
        };
        let san = format_move_as_san(&board, m);
        assert_eq!(san, "a8=Q");
    }

    #[test]
    fn test_san_queen_move() {
        // Queen on d1, move to h5
        let board: Board = "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2"
            .parse()
            .unwrap();
        let san = format_move_as_san(&board, mv(File::D, Rank::Eighth, File::H, Rank::Fourth));
        assert_eq!(san, "Qh4");
    }

    #[test]
    fn test_san_empty_square_fallback() {
        // Move from an empty square should fall back to UCI
        let board: Board = "8/8/8/8/8/8/8/4K2k w - - 0 1".parse().unwrap();
        let san = format_move_as_san(&board, mv(File::A, Rank::First, File::A, Rank::Second));
        // No piece on a1, should return UCI format
        assert_eq!(san, "a1a2");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("Illegal move")]
    IllegalMove,
    #[error("Nothing to undo")]
    NothingToUndo,
    #[error("Nothing to redo")]
    NothingToRedo,
    #[error("Not implemented")]
    NotImplemented,
    #[error("FEN parse error: {0}")]
    FenError(#[from] crate::fen::FenError),
}
