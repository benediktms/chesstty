use crate::chess::Game;
use crate::engine::{EngineHandle, EngineInfo};
use cozy_chess::{Color, Move, Square};

/// Main application state
pub struct AppState {
    pub game: Game,
    pub mode: GameMode,
    pub engine: Option<EngineHandle>,
    pub ui_state: UiState,
}

/// Game mode determines how the app behaves
#[derive(Debug, Clone, PartialEq)]
pub enum GameMode {
    HumanVsHuman,
    HumanVsEngine { human_side: Color },
    EngineVsEngine,
    AnalysisMode,
    ReviewMode,
}

/// UI-specific state (not part of game state)
pub struct UiState {
    pub selected_square: Option<Square>,
    pub highlighted_squares: Vec<Square>,
    pub last_move: Option<(Square, Square)>,
    pub engine_info: Option<EngineInfo>,
    pub status_message: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            game: Game::new(),
            mode: GameMode::HumanVsHuman,
            engine: None,
            ui_state: UiState {
                selected_square: None,
                highlighted_squares: Vec::new(),
                last_move: None,
                engine_info: None,
                status_message: None,
            },
        }
    }

    /// Select a square and highlight legal moves for the piece on it
    pub fn select_square(&mut self, square: Square) {
        // Check if there's a piece on this square of the current player's color
        let piece_color = self.game.position().color_on(square);
        let current_player = self.game.side_to_move();

        if let Some(color) = piece_color {
            if color == current_player {
                // Select this square and show legal moves
                self.ui_state.selected_square = Some(square);
                self.ui_state.highlighted_squares = self.get_legal_moves_for_square(square);
                self.ui_state.status_message = Some(format!("Selected {}", format_square(square)));
            } else {
                self.ui_state.status_message = Some("That's not your piece!".to_string());
            }
        } else {
            self.ui_state.status_message = Some("No piece on that square".to_string());
        }
    }

    /// Attempt to move the selected piece to the destination square
    pub fn try_move_to(&mut self, to_square: Square) -> Result<(), String> {
        if let Some(from_square) = self.ui_state.selected_square {
            // Check if this destination is in the highlighted (legal) moves
            if !self.ui_state.highlighted_squares.contains(&to_square) {
                return Err("Illegal move".to_string());
            }

            // Find the move that matches from -> to
            let legal_moves = self.game.legal_moves();
            let matching_move = legal_moves
                .iter()
                .find(|mv| mv.from == from_square && mv.to == to_square);

            if let Some(&mv) = matching_move {
                // Check if this is a pawn promotion
                let mv = if needs_promotion(&self.game, mv) {
                    // For now, always promote to queen
                    // TODO: Add promotion selection UI
                    Move {
                        from: mv.from,
                        to: mv.to,
                        promotion: Some(cozy_chess::Piece::Queen),
                    }
                } else {
                    mv
                };

                // Make the move
                self.game.make_move(mv).map_err(|e| e.to_string())?;

                // Update UI state
                self.ui_state.last_move = Some((from_square, to_square));
                self.ui_state.selected_square = None;
                self.ui_state.highlighted_squares.clear();
                self.ui_state.status_message = Some(format!(
                    "Moved {} to {}",
                    format_square(from_square),
                    format_square(to_square)
                ));

                Ok(())
            } else {
                Err("Move not found in legal moves".to_string())
            }
        } else {
            Err("No piece selected".to_string())
        }
    }

    /// Clear the current selection and highlights
    pub fn clear_selection(&mut self) {
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.status_message = None;
    }

    /// Clear all UI highlights and state
    pub fn clear_all_highlights(&mut self) {
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.status_message = None;
    }

    /// Get all legal destination squares for a piece on the given square
    fn get_legal_moves_for_square(&self, from_square: Square) -> Vec<Square> {
        self.game
            .legal_moves()
            .iter()
            .filter(|mv| mv.from == from_square)
            .map(|mv| mv.to)
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn format_square(sq: Square) -> String {
    let file = match sq.file() {
        cozy_chess::File::A => 'a',
        cozy_chess::File::B => 'b',
        cozy_chess::File::C => 'c',
        cozy_chess::File::D => 'd',
        cozy_chess::File::E => 'e',
        cozy_chess::File::F => 'f',
        cozy_chess::File::G => 'g',
        cozy_chess::File::H => 'h',
    };
    let rank = match sq.rank() {
        cozy_chess::Rank::First => '1',
        cozy_chess::Rank::Second => '2',
        cozy_chess::Rank::Third => '3',
        cozy_chess::Rank::Fourth => '4',
        cozy_chess::Rank::Fifth => '5',
        cozy_chess::Rank::Sixth => '6',
        cozy_chess::Rank::Seventh => '7',
        cozy_chess::Rank::Eighth => '8',
    };
    format!("{}{}", file, rank)
}

fn needs_promotion(game: &Game, mv: Move) -> bool {
    // Check if this is a pawn move to the last rank
    if let Some(piece) = game.position().piece_on(mv.from) {
        if piece == cozy_chess::Piece::Pawn {
            let to_rank = mv.to.rank();
            return to_rank == cozy_chess::Rank::Eighth || to_rank == cozy_chess::Rank::First;
        }
    }
    false
}
