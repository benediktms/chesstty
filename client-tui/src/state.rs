use crate::client::ChessClient;
use chess_proto::*;
use cozy_chess::{Board, Piece, Square};
use std::collections::HashMap;
use tonic::Streaming;

/// Client-side application state that communicates with the server
pub struct ClientState {
    pub client: ChessClient,
    pub mode: GameMode,
    pub skill_level: u8,
    pub ui_state: UiState,

    // Cached game state from server
    cached_fen: String,
    cached_board: Board, // Parsed board position for UI rendering
    cached_side_to_move: String,
    cached_status: i32,
    cached_history: Vec<MoveRecord>,
    legal_moves_cache: HashMap<String, Vec<MoveDetail>>,

    // Event streaming
    event_stream: Option<Streaming<GameEvent>>,
}

/// Game mode determines how the app behaves
#[derive(Debug, Clone, PartialEq)]
pub enum GameMode {
    HumanVsHuman,
    HumanVsEngine { human_side: PlayerColor },
    EngineVsEngine,
    AnalysisMode,
    ReviewMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerColor {
    White,
    Black,
}

/// UI-specific state (not part of game state)
pub struct UiState {
    pub selected_square: Option<Square>,
    pub highlighted_squares: Vec<Square>,
    pub selectable_squares: Vec<Square>,
    pub last_move: Option<(Square, Square)>,
    pub engine_info: Option<EngineInfo>,
    pub is_engine_thinking: bool,
    pub engine_move_triggered: bool,  // Track if we've triggered engine for current turn
    pub needs_refresh: bool,  // Track if we need to refresh state from server
    pub show_engine_panel: bool,
    pub status_message: Option<String>,
    pub input_phase: InputPhase,
    pub show_debug_panel: bool,
    pub uci_log: Vec<UciLogEntry>,
    pub selected_promotion_piece: Piece,
    pub move_history_scroll: u16,
    pub uci_debug_scroll: u16,
}

#[derive(Debug, Clone)]
pub struct UciLogEntry {
    pub direction: UciDirection,
    pub message: String,
    pub timestamp: std::time::Instant,
    pub move_context: Option<String>,
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

impl ClientState {
    /// Create a new client state (must connect to server first)
    pub async fn new(server_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client = ChessClient::connect(server_addr).await?;

        // Create a new session on the server
        let session_info = client.create_session(None).await?;

        // Parse the FEN to get the board
        let board = session_info.fen.parse::<Board>()
            .map_err(|e| format!("Failed to parse FEN: {}", e))?;

        let mut state = Self {
            client,
            mode: GameMode::HumanVsHuman,
            skill_level: 10,
            ui_state: UiState {
                selected_square: None,
                highlighted_squares: Vec::new(),
                selectable_squares: Vec::new(),
                last_move: None,
                engine_info: None,
                is_engine_thinking: false,
                engine_move_triggered: false,
                needs_refresh: false,
                show_engine_panel: true,  // Show by default when engine is active
                status_message: None,
                input_phase: InputPhase::SelectPiece,
                show_debug_panel: false,
                uci_log: Vec::new(),
                selected_promotion_piece: Piece::Queen,
                move_history_scroll: 0,
                uci_debug_scroll: 0,
            },
            cached_fen: session_info.fen.clone(),
            cached_board: board,
            cached_side_to_move: session_info.side_to_move.clone(),
            cached_status: session_info.status,
            cached_history: session_info.history,
            legal_moves_cache: HashMap::new(),
            event_stream: None,
        };

        state.update_selectable_squares().await?;
        Ok(state)
    }

    /// Get the current FEN position
    pub fn fen(&self) -> &str {
        &self.cached_fen
    }

    /// Get the current board position
    pub fn board(&self) -> &Board {
        &self.cached_board
    }

    /// Get the side to move
    pub fn side_to_move(&self) -> &str {
        &self.cached_side_to_move
    }

    /// Get game status
    pub fn status(&self) -> i32 {
        self.cached_status
    }

    /// Get move history
    pub fn history(&self) -> &[MoveRecord] {
        &self.cached_history
    }

    /// Check if it's the engine's turn to move
    pub fn is_engine_turn(&self) -> bool {
        match self.mode {
            GameMode::HumanVsEngine { human_side } => {
                let is_white_turn = self.cached_side_to_move == "white";
                match human_side {
                    PlayerColor::White => !is_white_turn,
                    PlayerColor::Black => is_white_turn,
                }
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

        // Check if we've already triggered the engine for this turn
        if self.ui_state.engine_move_triggered {
            return Ok(());
        }

        // Check if game is ongoing
        if self.cached_status != GameStatus::Ongoing as i32 {
            tracing::warn!("Game is not ongoing (status: {}), not triggering engine", self.cached_status);
            return Ok(());
        }

        tracing::info!("Triggering engine move (side to move: {})", self.cached_side_to_move);
        self.ui_state.status_message = Some("Engine thinking...".to_string());
        self.ui_state.engine_move_triggered = true;

        // Trigger engine move on server
        self.client
            .trigger_engine_move(None)
            .await
            .map_err(|e| {
                tracing::error!("Failed to trigger engine: {}", e);
                e.to_string()
            })?;

        Ok(())
    }

    /// Refresh game state from server (called after receiving MoveMadeEvent)
    pub async fn refresh_from_server(&mut self) -> Result<(), String> {
        let session_info = self.client.get_session().await.map_err(|e| e.to_string())?;
        self.update_from_session_info(session_info);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Update the list of squares with pieces that can be selected
    pub async fn update_selectable_squares(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::converters::parse_square;

        // Get all legal moves from server
        let moves = self.client.get_legal_moves(None).await?;

        // Extract unique "from" squares and convert to Square
        let mut from_squares: Vec<Square> = moves
            .iter()
            .filter_map(|m| parse_square(&m.from))
            .collect();
        from_squares.sort_by_key(|sq| (sq.rank() as u8, sq.file() as u8));
        from_squares.dedup();

        self.ui_state.selectable_squares = from_squares;

        // Cache legal moves by from square (still using string keys for lookup)
        self.legal_moves_cache.clear();
        for mv in moves {
            self.legal_moves_cache
                .entry(mv.from.clone())
                .or_insert_with(Vec::new)
                .push(mv);
        }

        Ok(())
    }

    /// Filter selectable squares by partial input (typeahead)
    pub fn filter_selectable_by_input(&self, input: &str) -> Vec<Square> {
        use crate::converters::format_square;

        if input.is_empty() {
            return vec![];
        }

        self.ui_state
            .selectable_squares
            .iter()
            .filter(|sq| format_square(**sq).starts_with(input))
            .copied()
            .collect()
    }

    /// Select a square and highlight legal moves for the piece on it
    pub fn select_square(&mut self, square: Square) {
        use crate::converters::{format_square, parse_square};

        // Check if this square is selectable
        if !self.ui_state.selectable_squares.contains(&square) {
            self.ui_state.status_message =
                Some("No piece on that square or not your turn".to_string());
            return;
        }

        // Get legal moves for this square from cache
        let square_str = format_square(square);
        if let Some(moves) = self.legal_moves_cache.get(&square_str) {
            self.ui_state.selected_square = Some(square);
            self.ui_state.highlighted_squares = moves
                .iter()
                .filter_map(|m| parse_square(&m.to))
                .collect();
            self.ui_state.input_phase = InputPhase::SelectDestination;
            self.ui_state.status_message = Some(format!("Selected {}", square_str));
        } else {
            self.ui_state.status_message = Some("No legal moves from that square".to_string());
        }
    }

    /// Attempt to move the selected piece to the destination square
    pub async fn try_move_to(&mut self, to_square: Square) -> Result<(), String> {
        use crate::converters::format_square;

        let from_square = self
            .ui_state
            .selected_square
            .ok_or("No piece selected")?;

        // Check if this destination is in the highlighted (legal) moves
        if !self.ui_state.highlighted_squares.contains(&to_square) {
            return Err("Illegal move".to_string());
        }

        let from_str = format_square(from_square);
        let to_str = format_square(to_square);

        // Check if this is a pawn promotion
        let needs_promotion = {
            if let Some(moves) = self.legal_moves_cache.get(&from_str) {
                moves
                    .iter()
                    .any(|m| m.to == to_str && m.promotion.is_some())
            } else {
                false
            }
        };

        if needs_promotion {
            // Transition to promotion selection phase
            self.ui_state.input_phase = InputPhase::SelectPromotion {
                from: from_square,
                to: to_square,
            };
            self.ui_state.selected_promotion_piece = Piece::Queen;
            self.ui_state.status_message = Some("Select promotion piece".to_string());
            return Ok(());
        }

        // Make the move on the server
        let response = self
            .client
            .make_move(&from_str, &to_str, None)
            .await
            .map_err(|e| e.to_string())?;

        // Update cached state
        if let Some(session_info) = response.session_info {
            self.update_from_session_info(session_info);
        }

        // Update UI state
        self.ui_state.last_move = Some((from_square, to_square));
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = Some(format!("Moved {} to {}", from_str, to_str));

        // Reset engine trigger flag so engine can move on its turn
        self.ui_state.engine_move_triggered = false;

        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Execute a promotion move after piece selection
    pub async fn execute_promotion(
        &mut self,
        from: Square,
        to: Square,
        piece: Piece,
    ) -> Result<(), String> {
        use crate::converters::{format_square, format_piece};

        let from_str = format_square(from);
        let to_str = format_square(to);
        let piece_str = format_piece(piece).to_string();

        // Make the move on the server with promotion
        let response = self
            .client
            .make_move(&from_str, &to_str, Some(piece_str.clone()))
            .await
            .map_err(|e| e.to_string())?;

        // Update cached state
        if let Some(session_info) = response.session_info {
            self.update_from_session_info(session_info);
        }

        // Update UI state
        self.ui_state.last_move = Some((from, to));
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = Some(format!("Promoted to {:?}", piece));

        // Reset engine trigger flag so engine can move on its turn
        self.ui_state.engine_move_triggered = false;

        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

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

    /// Clear the current selection and highlights
    pub fn clear_selection(&mut self) {
        self.ui_state.selected_square = None;
        self.ui_state.highlighted_squares.clear();
        self.ui_state.input_phase = InputPhase::SelectPiece;
        self.ui_state.status_message = None;
    }

    /// Clear all UI highlights and state
    pub fn clear_all_highlights(&mut self) {
        self.clear_selection();
    }

    /// Toggle debug panel visibility
    pub fn toggle_debug_panel(&mut self) {
        self.ui_state.show_debug_panel = !self.ui_state.show_debug_panel;
    }

    /// Toggle engine analysis panel visibility
    pub fn toggle_engine_panel(&mut self) {
        self.ui_state.show_engine_panel = !self.ui_state.show_engine_panel;
    }

    /// Subscribe to event stream from server
    pub async fn start_event_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.event_stream.is_none() {
            let stream = self.client.stream_events().await?;
            self.event_stream = Some(stream);
        }
        Ok(())
    }

    /// Poll for events from the stream (non-blocking)
    pub async fn poll_events(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use futures::StreamExt;

        if let Some(stream) = &mut self.event_stream {
            // Try to get next event without blocking
            match futures::poll!(stream.next()) {
                std::task::Poll::Ready(Some(result)) => {
                    match result {
                        Ok(event) => {
                            self.handle_event(event);
                        }
                        Err(e) => {
                            self.ui_state.status_message = Some(format!("Stream error: {}", e));
                            self.event_stream = None;
                        }
                    }
                }
                std::task::Poll::Ready(None) => {
                    // Stream ended
                    self.event_stream = None;
                }
                std::task::Poll::Pending => {
                    // No event available right now
                }
            }
        }
        Ok(())
    }

    /// Handle an event from the server
    fn handle_event(&mut self, event: GameEvent) {
        if let Some(event_type) = event.event {
            match event_type {
                game_event::Event::EngineThinking(thinking) => {
                    self.ui_state.engine_info = Some(thinking.info.unwrap_or_default());
                    self.ui_state.is_engine_thinking = true;
                }
                game_event::Event::EngineMoveReady(_ready) => {
                    // Server will execute engine moves automatically
                    // Just clear the thinking state
                    self.ui_state.is_engine_thinking = false;
                }
                game_event::Event::MoveMade(move_made) => {
                    // Move was made (either by human or engine)
                    if let Some(ref move_record) = move_made.r#move {
                        self.ui_state.status_message = Some(format!("Move: {}", move_record.san));
                        tracing::info!("Move made: {}", move_record.san);

                        // Update last move for highlighting
                        use crate::converters::parse_square;
                        if let (Some(from), Some(to)) = (
                            parse_square(&move_record.from),
                            parse_square(&move_record.to)
                        ) {
                            self.ui_state.last_move = Some((from, to));
                        }
                    }
                    self.ui_state.is_engine_thinking = false;
                    self.ui_state.engine_move_triggered = false;

                    // Request refresh from server to get updated state
                    self.ui_state.needs_refresh = true;
                }
                game_event::Event::GameEnded(ended) => {
                    self.ui_state.status_message = Some(format!(
                        "Game ended: {} - {}",
                        ended.result, ended.reason
                    ));
                    self.ui_state.is_engine_thinking = false;
                    tracing::info!("Game ended: {} - {}", ended.result, ended.reason);
                }
                game_event::Event::Error(error) => {
                    let error_msg = format!("Server error: {}", error.error_message);
                    tracing::error!("{}", error_msg);
                    self.ui_state.status_message = Some(error_msg);
                    self.ui_state.is_engine_thinking = false;
                    self.ui_state.engine_move_triggered = false;
                }
                game_event::Event::UciMessage(uci_msg) => {
                    let direction = match uci_msg.direction {
                        0 => UciDirection::ToEngine,
                        1 => UciDirection::FromEngine,
                        _ => UciDirection::FromEngine, // default fallback
                    };
                    self.log_uci_message(direction, uci_msg.message, uci_msg.context);
                }
            }
        }
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

        // Keep only last 100 messages
        if self.ui_state.uci_log.len() > 100 {
            self.ui_state.uci_log.remove(0);
        }
    }

    /// Undo the last move
    pub async fn undo(&mut self) -> Result<(), String> {
        let session_info = self.client.undo_move().await.map_err(|e| e.to_string())?;

        self.update_from_session_info(session_info);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        // Reset engine trigger flag
        self.ui_state.engine_move_triggered = false;

        self.ui_state.status_message = Some("Move undone".to_string());
        Ok(())
    }

    /// Reset the game
    pub async fn reset(&mut self, fen: Option<String>) -> Result<(), String> {
        let session_info = self
            .client
            .reset_game(fen)
            .await
            .map_err(|e| e.to_string())?;

        self.update_from_session_info(session_info);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        self.clear_selection();

        // Reset engine trigger flag
        self.ui_state.engine_move_triggered = false;

        self.ui_state.status_message = Some("Game reset".to_string());
        Ok(())
    }

    /// Set engine configuration
    pub async fn set_engine(&mut self, enabled: bool, skill_level: u8) -> Result<(), String> {
        self.skill_level = skill_level;

        self.client
            .set_engine(enabled, skill_level as u32)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Update cached state from server SessionInfo
    fn update_from_session_info(&mut self, info: SessionInfo) {
        self.cached_fen = info.fen.clone();

        // Parse the new FEN to update the board
        if let Ok(board) = info.fen.parse::<Board>() {
            self.cached_board = board;
        } else {
            tracing::error!("Failed to parse FEN from server: {}", info.fen);
        }

        self.cached_side_to_move = info.side_to_move;
        self.cached_status = info.status;
        self.cached_history = info.history;
    }

    /// Refresh state from server
    pub async fn refresh(&mut self) -> Result<(), String> {
        let session_info = self.client.get_session().await.map_err(|e| e.to_string())?;

        self.update_from_session_info(session_info);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

impl Drop for ClientState {
    fn drop(&mut self) {
        // Best effort to close the session
        // We can't await here, so we'll just drop the connection
    }
}
