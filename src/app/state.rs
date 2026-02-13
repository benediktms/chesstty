use crate::chess::Game;
use crate::engine::{EngineEvent, EngineHandle, EngineInfo, StockfishEngine};
use cozy_chess::{Color, Move, Square};

/// Main application state
pub struct AppState {
    pub game: Game,
    pub mode: GameMode,
    pub engine: Option<StockfishEngine>,
    pub skill_level: u8,
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
    pub selectable_squares: Vec<Square>,  // Squares with pieces that can be selected
    pub last_move: Option<(Square, Square)>,
    pub engine_info: Option<EngineInfo>,
    pub status_message: Option<String>,
    pub input_phase: InputPhase,  // Which input box is active
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputPhase {
    SelectPiece,
    SelectDestination,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            game: Game::new(),
            mode: GameMode::HumanVsHuman,
            engine: None,
            skill_level: 10, // Default intermediate
            ui_state: UiState {
                selected_square: None,
                highlighted_squares: Vec::new(),
                selectable_squares: Vec::new(),
                last_move: None,
                engine_info: None,
                status_message: None,
                input_phase: InputPhase::SelectPiece,
            },
        };
        state.update_selectable_squares();
        state
    }

    /// Check if it's the engine's turn to move
    pub fn is_engine_turn(&self) -> bool {
        match self.mode {
            GameMode::HumanVsEngine { human_side } => {
                self.game.side_to_move() != human_side
            }
            GameMode::EngineVsEngine => true,
            _ => false,
        }
    }

    /// Make a move with the engine
    pub async fn make_engine_move(&mut self) -> Result<(), String> {
        if !self.is_engine_turn() {
            return Ok(());
        }

        // Check if game is over
        if !matches!(self.game.status(), cozy_chess::GameStatus::Ongoing) {
            return Ok(());
        }

        self.ui_state.status_message = Some("Engine thinking...".to_string());

        // Get current position as FEN
        let fen = self.game.to_fen();
        let moves: Vec<Move> = self.game.history().iter().map(|e| e.mv).collect();

        // Make sure we have an engine
        if self.engine.is_none() {
            return Err("Engine not initialized".to_string());
        }

        let engine = self.engine.as_ref().unwrap();

        // Send position to engine
        engine
            .send_command(crate::engine::EngineCommand::SetPosition {
                fen,
                moves: vec![], // Already included in FEN
            })
            .await?;

        // Calculate move time based on skill level
        let movetime = match self.skill_level {
            0..=5 => 200,     // Beginner: 200ms
            6..=10 => 500,    // Intermediate: 500ms
            11..=15 => 1000,  // Advanced: 1s
            _ => 2000,        // Master: 2s
        };

        // Start calculation
        engine
            .send_command(crate::engine::EngineCommand::Go(
                crate::engine::GoParams {
                    movetime: Some(movetime),
                    depth: None,
                    infinite: false,
                },
            ))
            .await?;

        Ok(())
    }

    /// Process engine events (call this in the main loop)
    pub fn process_engine_events(&mut self) -> Option<Move> {
        if let Some(engine) = &mut self.engine {
            while let Some(event) = engine.try_recv_event() {
                match event {
                    EngineEvent::BestMove(mv) => {
                        return Some(mv);
                    }
                    EngineEvent::Info(info) => {
                        self.ui_state.engine_info = Some(info);
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Update the list of squares with pieces that can be selected
    pub fn update_selectable_squares(&mut self) {
        let current_color = self.game.side_to_move();
        self.ui_state.selectable_squares.clear();

        // Find all squares with pieces of the current player's color
        for rank in 0..8 {
            for file in 0..8 {
                let square = Square::new(
                    cozy_chess::File::index(file),
                    cozy_chess::Rank::index(rank),
                );
                if let Some(piece_color) = self.game.position().color_on(square) {
                    if piece_color == current_color {
                        // Check if this piece has any legal moves
                        let has_moves = self
                            .game
                            .legal_moves()
                            .iter()
                            .any(|mv| mv.from == square);
                        if has_moves {
                            self.ui_state.selectable_squares.push(square);
                        }
                    }
                }
            }
        }
    }

    /// Filter selectable squares by partial input (typeahead)
    pub fn filter_selectable_by_input(&self, input: &str) -> Vec<Square> {
        if input.is_empty() {
            return self.ui_state.selectable_squares.clone();
        }

        let chars: Vec<char> = input.chars().collect();

        // Filter by file (first character)
        let file_filter = match chars.get(0) {
            Some(&'a') => Some(cozy_chess::File::A),
            Some(&'b') => Some(cozy_chess::File::B),
            Some(&'c') => Some(cozy_chess::File::C),
            Some(&'d') => Some(cozy_chess::File::D),
            Some(&'e') => Some(cozy_chess::File::E),
            Some(&'f') => Some(cozy_chess::File::F),
            Some(&'g') => Some(cozy_chess::File::G),
            Some(&'h') => Some(cozy_chess::File::H),
            _ => None,
        };

        let mut filtered: Vec<Square> = self.ui_state.selectable_squares
            .iter()
            .filter(|sq| {
                if let Some(file) = file_filter {
                    sq.file() == file
                } else {
                    false
                }
            })
            .copied()
            .collect();

        // If we have a rank too (second character), filter further
        if let Some(&rank_char) = chars.get(1) {
            let rank_filter = match rank_char {
                '1' => Some(cozy_chess::Rank::First),
                '2' => Some(cozy_chess::Rank::Second),
                '3' => Some(cozy_chess::Rank::Third),
                '4' => Some(cozy_chess::Rank::Fourth),
                '5' => Some(cozy_chess::Rank::Fifth),
                '6' => Some(cozy_chess::Rank::Sixth),
                '7' => Some(cozy_chess::Rank::Seventh),
                '8' => Some(cozy_chess::Rank::Eighth),
                _ => None,
            };

            if let Some(rank) = rank_filter {
                filtered.retain(|sq| sq.rank() == rank);
            }
        }

        filtered
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
                self.ui_state.input_phase = InputPhase::SelectDestination;
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
                self.ui_state.input_phase = InputPhase::SelectPiece;
                self.ui_state.status_message = Some(format!(
                    "Moved {} to {}",
                    format_square(from_square),
                    format_square(to_square)
                ));
                self.update_selectable_squares();

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
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = None;
        self.update_selectable_squares();
    }

    /// Clear all UI highlights and state
    pub fn clear_all_highlights(&mut self) {
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = None;
        self.update_selectable_squares();
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
