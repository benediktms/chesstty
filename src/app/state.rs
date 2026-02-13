use crate::chess::Game;
use crate::engine::{EngineEvent, EngineHandle, EngineInfo, StockfishEngine};
use cozy_chess::{Color, Move, Piece, Square};

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
    pub selectable_squares: Vec<Square>, // Squares with pieces that can be selected
    pub last_move: Option<(Square, Square)>,
    pub engine_info: Option<EngineInfo>,
    pub status_message: Option<String>,
    pub input_phase: InputPhase,           // Which input box is active
    pub show_debug_panel: bool,            // Toggle UCI debug panel
    pub uci_log: Vec<UciLogEntry>,         // UCI message log
    pub selected_promotion_piece: Piece,   // Currently selected piece for promotion (default: Queen)
}

#[derive(Debug, Clone)]
pub struct UciLogEntry {
    pub direction: UciDirection,
    pub message: String,
    pub timestamp: std::time::Instant,
    pub move_context: Option<String>, // Associated move if relevant
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UciDirection {
    ToEngine,
    FromEngine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputPhase {
    SelectPiece,
    SelectDestination,
    SelectPromotion { from: Square, to: Square },
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
                show_debug_panel: false,
                uci_log: Vec::new(),
                selected_promotion_piece: Piece::Queen,
            },
        };
        state.update_selectable_squares();
        state
    }

    /// Check if it's the engine's turn to move
    pub fn is_engine_turn(&self) -> bool {
        match self.mode {
            GameMode::HumanVsEngine { human_side } => self.game.side_to_move() != human_side,
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

        // Make sure we have an engine
        if self.engine.is_none() {
            return Err("Engine not initialized".to_string());
        }

        // Calculate move time based on skill level
        let movetime = match self.skill_level {
            0..=5 => 200,    // Beginner: 200ms
            6..=10 => 500,   // Intermediate: 500ms
            11..=15 => 1000, // Advanced: 1s
            _ => 2000,       // Master: 2s
        };

        // Log position command
        self.log_uci_message(
            UciDirection::ToEngine,
            format!("position fen {}", fen),
            None,
        );

        // Send position to engine
        {
            let engine = self.engine.as_ref().unwrap();
            engine
                .send_command(crate::engine::EngineCommand::SetPosition {
                    fen,
                    moves: vec![], // Already included in FEN
                })
                .await?;
        }

        // Log go command
        self.log_uci_message(
            UciDirection::ToEngine,
            format!("go movetime {}", movetime),
            Some(format!("Move #{}", self.game.history().len() + 1)),
        );

        // Start calculation
        {
            let engine = self.engine.as_ref().unwrap();
            engine
                .send_command(crate::engine::EngineCommand::Go(crate::engine::GoParams {
                    movetime: Some(movetime),
                    depth: None,
                    infinite: false,
                }))
                .await?;
        }

        Ok(())
    }

    /// Log a UCI message
    pub fn log_uci_message(
        &mut self,
        direction: UciDirection,
        message: String,
        move_context: Option<String>,
    ) {
        self.ui_state.uci_log.push(UciLogEntry {
            direction,
            message,
            timestamp: std::time::Instant::now(),
            move_context,
        });

        // Keep only last 100 messages to avoid memory issues
        if self.ui_state.uci_log.len() > 100 {
            self.ui_state.uci_log.remove(0);
        }
    }

    /// Toggle debug panel visibility
    pub fn toggle_debug_panel(&mut self) {
        self.ui_state.show_debug_panel = !self.ui_state.show_debug_panel;
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
                let square =
                    Square::new(cozy_chess::File::index(file), cozy_chess::Rank::index(rank));
                if let Some(piece_color) = self.game.position().color_on(square) {
                    if piece_color == current_color {
                        // Check if this piece has any legal moves
                        let has_moves = self.game.legal_moves().iter().any(|mv| mv.from == square);
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
            return vec![];
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

        let mut filtered: Vec<Square> = self
            .ui_state
            .selectable_squares
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
                if needs_promotion(&self.game, mv) {
                    // Transition to promotion selection phase
                    self.ui_state.input_phase = InputPhase::SelectPromotion {
                        from: from_square,
                        to: to_square,
                    };
                    self.ui_state.selected_promotion_piece = Piece::Queen;
                    self.ui_state.status_message = Some("Select promotion piece".to_string());
                    return Ok(());
                }

                // Non-promotion move - execute immediately
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

    /// Execute a promotion move after piece selection
    pub fn execute_promotion(&mut self, from: Square, to: Square, piece: Piece) -> Result<(), String> {
        let mv = Move {
            from,
            to,
            promotion: Some(piece),
        };

        // Validate this specific promotion move is legal
        if !self.game.legal_moves().contains(&mv) {
            return Err("Invalid promotion move".to_string());
        }

        self.game.make_move(mv).map_err(|e| e.to_string())?;

        // Update UI state
        self.ui_state.last_move = Some((from, to));
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = Some(format!(
            "Promoted to {}",
            format_piece_name(piece)
        ));
        self.update_selectable_squares();

        Ok(())
    }

    /// Cancel promotion selection and return to piece selection
    pub fn cancel_promotion(&mut self) {
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = Some("Promotion cancelled".to_string());
    }

    /// Cycle promotion piece selection
    pub fn cycle_promotion_piece(&mut self, direction: i8) {
        let pieces = [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];

        let current_idx = pieces
            .iter()
            .position(|&p| p == self.ui_state.selected_promotion_piece)
            .unwrap_or(0);

        let new_idx = if direction > 0 {
            (current_idx + 1) % pieces.len()
        } else if direction < 0 {
            (current_idx + pieces.len() - 1) % pieces.len()
        } else {
            current_idx
        };

        self.ui_state.selected_promotion_piece = pieces[new_idx];
    }

    /// Set promotion piece directly
    pub fn set_promotion_piece(&mut self, piece: Piece) {
        self.ui_state.selected_promotion_piece = piece;
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

fn format_piece_name(piece: Piece) -> &'static str {
    match piece {
        Piece::Queen => "Queen",
        Piece::Rook => "Rook",
        Piece::Bishop => "Bishop",
        Piece::Knight => "Knight",
        Piece::King => "King",
        Piece::Pawn => "Pawn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_promotion_white_eighth_rank() {
        // White pawn on e7 moving to e8 should need promotion
        let fen = "4k3/4P3/8/8/8/8/8/4K3 w - - 0 1";
        let game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Eighth);
        let mv = Move {
            from,
            to,
            promotion: None,
        };

        assert!(needs_promotion(&game, mv));
    }

    #[test]
    fn test_needs_promotion_black_first_rank() {
        // Black pawn on e2 moving to e1 should need promotion
        let fen = "4k3/8/8/8/8/8/4p3/4K3 b - - 0 1";
        let game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Second);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::First);
        let mv = Move {
            from,
            to,
            promotion: None,
        };

        assert!(needs_promotion(&game, mv));
    }

    #[test]
    fn test_needs_promotion_not_last_rank() {
        // White pawn on e2 moving to e3 should not need promotion
        let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
        let game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Second);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Third);
        let mv = Move {
            from,
            to,
            promotion: None,
        };

        assert!(!needs_promotion(&game, mv));
    }

    #[test]
    fn test_needs_promotion_not_pawn() {
        // Knight moving to 8th rank should not need promotion
        let fen = "4k3/4N3/8/8/8/8/8/4K3 w - - 0 1";
        let game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Eighth);
        let mv = Move {
            from,
            to,
            promotion: None,
        };

        assert!(!needs_promotion(&game, mv));
    }

    #[test]
    fn test_cycle_promotion_piece_forward() {
        let mut state = AppState::new();
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Queen);

        state.cycle_promotion_piece(1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Rook);

        state.cycle_promotion_piece(1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Bishop);

        state.cycle_promotion_piece(1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Knight);

        state.cycle_promotion_piece(1); // Wrap around
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Queen);
    }

    #[test]
    fn test_cycle_promotion_piece_backward() {
        let mut state = AppState::new();
        state.ui_state.selected_promotion_piece = Piece::Queen;

        state.cycle_promotion_piece(-1); // Should wrap to Knight
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Knight);

        state.cycle_promotion_piece(-1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Bishop);

        state.cycle_promotion_piece(-1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Rook);

        state.cycle_promotion_piece(-1);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Queen);
    }

    #[test]
    fn test_set_promotion_piece() {
        let mut state = AppState::new();

        state.set_promotion_piece(Piece::Knight);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Knight);

        state.set_promotion_piece(Piece::Rook);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Rook);

        state.set_promotion_piece(Piece::Bishop);
        assert_eq!(state.ui_state.selected_promotion_piece, Piece::Bishop);
    }

    #[test]
    fn test_cancel_promotion() {
        let mut state = AppState::new();
        state.ui_state.input_phase = InputPhase::SelectPromotion {
            from: Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh),
            to: Square::new(cozy_chess::File::E, cozy_chess::Rank::Eighth),
        };
        state.ui_state.selected_square = Some(Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh));

        state.cancel_promotion();

        assert_eq!(state.ui_state.input_phase, InputPhase::SelectPiece);
        assert_eq!(state.ui_state.selected_square, None);
    }

    #[test]
    fn test_execute_promotion_queen() {
        // Set up position with white pawn on e7, can promote to e8
        let fen = "8/4P2k/8/8/8/8/8/4K3 w - - 0 1";
        let mut state = AppState::new();
        state.game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Eighth);

        // Execute promotion to Queen
        let result = state.execute_promotion(from, to, Piece::Queen);
        assert!(result.is_ok(), "Failed to execute promotion: {:?}", result);

        // Verify the piece is now a Queen on e8
        assert_eq!(
            state.game.position().piece_on(to),
            Some(Piece::Queen)
        );
    }

    #[test]
    fn test_execute_promotion_knight() {
        // Set up position with white pawn on e7, can promote to e8
        let fen = "8/4P2k/8/8/8/8/8/4K3 w - - 0 1";
        let mut state = AppState::new();
        state.game = Game::from_fen(fen).unwrap();

        let from = Square::new(cozy_chess::File::E, cozy_chess::Rank::Seventh);
        let to = Square::new(cozy_chess::File::E, cozy_chess::Rank::Eighth);

        // Execute promotion to Knight (underpromotion)
        let result = state.execute_promotion(from, to, Piece::Knight);
        assert!(result.is_ok(), "Failed to execute promotion: {:?}", result);

        // Verify the piece is now a Knight on e8
        assert_eq!(
            state.game.position().piece_on(to),
            Some(Piece::Knight)
        );
    }
}
