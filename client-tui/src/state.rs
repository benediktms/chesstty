use crate::review_state::ReviewState;
use chess_client::ChessClient;
use chess_client::*;
use cozy_chess::{Board, Piece, Square};
use std::collections::HashMap;
use std::path::PathBuf;
use tonic::Streaming;

/// Get the socket path for server communication.
fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SOCKET_PATH") {
        return PathBuf::from(path);
    }
    PathBuf::from("/tmp/chesstty.sock")
}

/// Convert a proto GameModeProto to the client's local GameMode.
pub fn game_mode_from_proto(proto: &GameModeProto) -> GameMode {
    match GameModeType::try_from(proto.mode) {
        Ok(GameModeType::HumanVsHuman) => GameMode::HumanVsHuman,
        Ok(GameModeType::HumanVsEngine) => {
            let human_side = match proto
                .human_side
                .and_then(|v| PlayerSideProto::try_from(v).ok())
            {
                Some(PlayerSideProto::Black) => PlayerColor::Black,
                _ => PlayerColor::White,
            };
            GameMode::HumanVsEngine { human_side }
        }
        Ok(GameModeType::EngineVsEngine) => GameMode::EngineVsEngine,
        Ok(GameModeType::Analysis) => GameMode::AnalysisMode,
        Ok(GameModeType::Review) => GameMode::ReviewMode,
        Err(_) => GameMode::HumanVsHuman,
    }
}

/// A game session - connection to the server and current game state.
/// The server is the source of truth — the client stores the latest
/// snapshot and renders it.
pub struct GameSession {
    pub client: ChessClient,
    pub mode: GameMode,
    pub skill_level: u8,

    /// Engine analysis info from the server
    pub engine_info: Option<EngineInfo>,
    /// Whether the engine is currently thinking
    pub is_engine_thinking: bool,
    /// UCI log entries
    pub uci_log: Vec<UciLogEntry>,
    /// Game paused state (from server)
    pub paused: bool,
    /// Paused state before entering menu
    pub paused_before_menu: bool,

    /// Selected square during move input
    pub selected_square: Option<Square>,
    /// Highlighted squares (e.g., last move, legal moves)
    pub highlighted_squares: Vec<Square>,
    /// Selectable squares for current input phase
    pub selectable_squares: Vec<Square>,
    /// Last move made
    pub last_move: Option<(Square, Square)>,
    /// Best move squares (from engine analysis)
    pub best_move_squares: Option<(Square, Square)>,
    /// Selected promotion piece
    pub selected_promotion_piece: Piece,
    /// Status message to display
    pub status_message: Option<String>,

    /// The latest snapshot from the server — single source of truth.
    pub snapshot: SessionSnapshot,
    /// Board parsed from snapshot.fen for rendering.
    board: Board,
    /// Legal moves from the server, cached for interaction.
    legal_moves_cache: HashMap<String, Vec<MoveDetail>>,

    /// Event streaming
    event_stream: Option<Streaming<SessionStreamEvent>>,

    /// Review mode state (populated when viewing a post-game review).
    pub review_state: Option<ReviewState>,

    /// Pre-history moves from a snapshot (moves played before the snapshot position).
    /// Displayed before the current game's move history in the move history panel.
    pub pre_history: Vec<MoveRecord>,
}

/// Game mode determines how the app behaves
#[derive(Debug, Clone, PartialEq, Default)]
pub enum GameMode {
    #[default]
    HumanVsHuman,
    HumanVsEngine {
        human_side: PlayerColor,
    },
    EngineVsEngine,
    AnalysisMode,
    ReviewMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerColor {
    White,
    Black,
}

/// Render state - all UI state needed to render the interface.
/// This is produced by the FSM and consumed by the renderer.
///
/// NOTE: This struct contains both game state (from server) and UI state.
/// Over time, UI state should move to UiStateMachine.
///
/// EDIT: Now deleted - UI state is in UiStateMachine, game state is in GameSession.

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

impl GameSession {
    /// Create a new client state and session on the server.
    pub async fn new(
        server_addr: &str,
        fen: Option<String>,
        game_mode_proto: Option<GameModeProto>,
        timer: Option<TimerState>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client = ChessClient::connect_uds(&get_socket_path()).await?;
        let snapshot = client.create_session(fen, game_mode_proto, timer).await?;

        let board = snapshot
            .fen
            .parse::<Board>()
            .map_err(|e| format!("Failed to parse FEN: {}", e))?;

        let mode = snapshot
            .game_mode
            .as_ref()
            .map(game_mode_from_proto)
            .unwrap_or(GameMode::HumanVsHuman);

        let mut state = Self {
            client,
            mode,
            skill_level: 10,
            // Engine state
            engine_info: None,
            is_engine_thinking: false,
            uci_log: Vec::new(),
            paused: false,
            paused_before_menu: false,
            // Board state
            selected_square: None,
            highlighted_squares: Vec::new(),
            selectable_squares: Vec::new(),
            last_move: None,
            best_move_squares: None,
            selected_promotion_piece: Piece::Queen,
            status_message: None,
            // Snapshot and board
            snapshot,
            board,
            legal_moves_cache: HashMap::new(),
            event_stream: None,
            review_state: None,
            pre_history: Vec::new(),
        };

        state.update_selectable_squares().await?;
        Ok(state)
    }

    /// Create a client state for review mode (no server session created).
    pub async fn new_review(
        server_addr: &str,
        review: GameReviewProto,
        review_game_mode: Option<GameModeProto>,
        review_skill_level: u8,
        advanced: Option<AdvancedGameAnalysisProto>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = ChessClient::connect_uds(&get_socket_path()).await?;

        let board = Board::default();
        let snapshot = SessionSnapshot::default();

        Ok(Self {
            client,
            mode: GameMode::ReviewMode,
            skill_level: 0,
            // Engine state - not used in review
            engine_info: None,
            is_engine_thinking: false,
            uci_log: Vec::new(),
            paused: false,
            paused_before_menu: false,
            // Board state
            selected_square: None,
            highlighted_squares: Vec::new(),
            selectable_squares: Vec::new(),
            last_move: None,
            best_move_squares: None,
            selected_promotion_piece: Piece::Queen,
            status_message: Some("Review mode - use arrow keys to navigate".to_string()),
            // Snapshot and board
            snapshot,
            board,
            legal_moves_cache: HashMap::new(),
            event_stream: None,
            review_state: Some(ReviewState::with_metadata(
                review,
                review_game_mode,
                review_skill_level,
                advanced,
            )),
            pre_history: Vec::new(),
        })
    }

    // --- Accessors: read from snapshot ---

    pub fn fen(&self) -> &str {
        if let Some(ref rs) = self.review_state {
            &rs.fen_at_ply
        } else {
            &self.snapshot.fen
        }
    }

    pub fn board(&self) -> &Board {
        if let Some(ref rs) = self.review_state {
            &rs.board_at_ply
        } else {
            &self.board
        }
    }

    pub fn side_to_move(&self) -> &str {
        if let Some(ref rs) = self.review_state {
            rs.side_to_move()
        } else {
            &self.snapshot.side_to_move
        }
    }

    pub fn status(&self) -> i32 {
        self.snapshot.status
    }

    pub fn history(&self) -> &[MoveRecord] {
        if let Some(ref rs) = self.review_state {
            &rs.move_history
        } else {
            &self.snapshot.history
        }
    }

    pub fn is_undo_allowed(&self) -> bool {
        matches!(self.mode, GameMode::HumanVsEngine { .. }) && self.skill_level <= 3
    }

    // --- Server communication ---

    pub async fn refresh_from_server(&mut self) -> Result<(), String> {
        let snapshot = self.client.get_session().await.map_err(|e| e.to_string())?;
        self.apply_snapshot(snapshot);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn update_selectable_squares(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use ::chess::parse_square;

        let moves = self.client.get_legal_moves(None).await?;

        let mut from_squares: Vec<Square> =
            moves.iter().filter_map(|m| parse_square(&m.from)).collect();
        from_squares.sort_by_key(|sq| (sq.rank() as u8, sq.file() as u8));
        from_squares.dedup();

        self.selectable_squares = from_squares;

        self.legal_moves_cache.clear();
        for mv in moves {
            self.legal_moves_cache
                .entry(mv.from.clone())
                .or_default()
                .push(mv);
        }

        Ok(())
    }

    pub fn filter_selectable_by_input(&self, input: &str) -> Vec<Square> {
        use ::chess::format_square;
        if input.is_empty() {
            return vec![];
        }
        self.selectable_squares
            .iter()
            .filter(|sq| format_square(**sq).starts_with(input))
            .copied()
            .collect()
    }

    pub fn legal_moves_from(&self, from_square: Square) -> Option<Vec<MoveDetail>> {
        use ::chess::format_square;
        let from_str = format_square(from_square);
        self.legal_moves_cache.get(&from_str).cloned()
    }

    pub fn select_square(&mut self, square: Square) {
        use ::chess::{format_square, parse_square};

        if !self.selectable_squares.contains(&square) {
            self.status_message = Some("No piece on that square or not your turn".to_string());
            return;
        }

        let square_str = format_square(square);
        if let Some(moves) = self.legal_moves_cache.get(&square_str) {
            self.selected_square = Some(square);
            self.highlighted_squares = moves.iter().filter_map(|m| parse_square(&m.to)).collect();
            // input_phase now handled by FSM
            self.status_message = Some(format!("Selected {}", square_str));
        } else {
            self.status_message = Some("No legal moves from that square".to_string());
        }
    }

    pub async fn try_move_to(&mut self, to_square: Square) -> Result<(), String> {
        use ::chess::format_square;

        let from_square = self.selected_square.ok_or("No piece selected")?;

        if !self.highlighted_squares.contains(&to_square) {
            return Err("Illegal move".to_string());
        }

        let from_str = format_square(from_square);
        let to_str = format_square(to_square);

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
            // input_phase now handled by FSM - select promotion piece in FSM
            self.selected_promotion_piece = Piece::Queen;
            self.status_message = Some("Select promotion piece".to_string());
            return Ok(());
        }

        let snapshot = self
            .client
            .make_move(&from_str, &to_str, None)
            .await
            .map_err(|e| e.to_string())?;

        self.apply_snapshot(snapshot);

        self.last_move = Some((from_square, to_square));
        self.selected_square = None;
        self.highlighted_squares.clear();
        // input_phase now handled by FSM
        self.status_message = Some(format!("Moved {} to {}", from_str, to_str));

        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn execute_promotion(
        &mut self,
        from: Square,
        to: Square,
        piece: Piece,
    ) -> Result<(), String> {
        use ::chess::{format_piece, format_square};

        let from_str = format_square(from);
        let to_str = format_square(to);
        let piece_str = format_piece(piece).to_string();

        let snapshot = self
            .client
            .make_move(&from_str, &to_str, Some(piece_str))
            .await
            .map_err(|e| e.to_string())?;

        self.apply_snapshot(snapshot);

        self.last_move = Some((from, to));
        self.selected_square = None;
        self.highlighted_squares.clear();
        // input_phase now handled by FSM
        self.status_message = Some(format!("Promoted to {:?}", piece));

        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selected_square = None;
        self.highlighted_squares.clear();
        // input_phase now handled by FSM
        self.status_message = None;
    }

    // --- Event streaming ---

    pub async fn start_event_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.event_stream.is_none() {
            let stream = self.client.stream_events().await?;
            self.event_stream = Some(stream);
        }
        Ok(())
    }

    pub async fn poll_event_async(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use futures::StreamExt;

        if let Some(stream) = &mut self.event_stream {
            match stream.next().await {
                Some(Ok(event)) => {
                    self.handle_event(event).await;
                    Ok(())
                }
                Some(Err(e)) => {
                    self.status_message = Some(format!("Stream error: {}", e));
                    self.event_stream = None;
                    Err(e.into())
                }
                None => {
                    self.event_stream = None;
                    Err("Event stream ended".into())
                }
            }
        } else {
            std::future::pending::<()>().await;
            Ok(())
        }
    }

    pub async fn poll_events(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        use futures::StreamExt;

        if let Some(stream) = &mut self.event_stream {
            match futures::poll!(stream.next()) {
                std::task::Poll::Ready(Some(result)) => match result {
                    Ok(event) => {
                        self.handle_event(event).await;
                        Ok(true)
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Stream error: {}", e));
                        self.event_stream = None;
                        Err(e.into())
                    }
                },
                std::task::Poll::Ready(None) => {
                    self.event_stream = None;
                    Ok(false)
                }
                std::task::Poll::Pending => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    async fn handle_event(&mut self, event: SessionStreamEvent) {
        if let Some(event_type) = event.event {
            match event_type {
                session_stream_event::Event::StateChanged(snapshot) => {
                    tracing::info!("State changed: fen={}", snapshot.fen);

                    if let Some(ref last_move) = snapshot.last_move {
                        use ::chess::parse_square;
                        if let (Some(from), Some(to)) =
                            (parse_square(&last_move.from), parse_square(&last_move.to))
                        {
                            self.last_move = Some((from, to));
                        }
                    }

                    self.is_engine_thinking = snapshot.engine_thinking;
                    self.apply_snapshot(snapshot);

                    if let Err(e) = self.update_selectable_squares().await {
                        tracing::warn!(
                            "Failed to update selectable squares after state change: {}",
                            e
                        );
                    }
                }
                session_stream_event::Event::EngineThinking(analysis) => {
                    let info = EngineInfo {
                        depth: analysis.depth,
                        seldepth: analysis.seldepth,
                        time_ms: analysis.time_ms,
                        nodes: analysis.nodes,
                        score: analysis.score.clone(),
                        pv: analysis.pv.clone(),
                        nps: analysis.nps,
                    };
                    self.engine_info = Some(info);
                    self.is_engine_thinking = true;
                }
                session_stream_event::Event::UciMessage(uci_msg) => {
                    let direction = match uci_msg.direction {
                        0 => UciDirection::ToEngine,
                        1 => UciDirection::FromEngine,
                        _ => UciDirection::FromEngine,
                    };
                    self.log_uci_message(direction, uci_msg.message, uci_msg.context);
                }
                session_stream_event::Event::Error(err_string) => {
                    let error_msg = format!("Server error: {}", err_string);
                    tracing::error!("{}", error_msg);
                    self.status_message = Some(error_msg);
                    self.is_engine_thinking = false;
                }
            }
        }
    }

    pub fn log_uci_message(
        &mut self,
        direction: UciDirection,
        message: String,
        move_context: Option<String>,
    ) {
        self.uci_log.push(UciLogEntry {
            direction,
            message,
            timestamp: std::time::Instant::now(),
            move_context,
        });
        if self.uci_log.len() > 100 {
            self.uci_log.remove(0);
        }
    }

    // --- Game actions ---

    pub async fn undo(&mut self) -> Result<(), String> {
        let snapshot = self.client.undo_move().await.map_err(|e| e.to_string())?;
        self.apply_snapshot(snapshot);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;
        self.status_message = Some("Move undone".to_string());
        Ok(())
    }

    pub async fn reset(&mut self, fen: Option<String>) -> Result<(), String> {
        let snapshot = self
            .client
            .reset_game(fen)
            .await
            .map_err(|e| e.to_string())?;
        self.apply_snapshot(snapshot);
        self.update_selectable_squares()
            .await
            .map_err(|e| e.to_string())?;
        self.clear_selection();
        self.status_message = Some("Game reset".to_string());
        Ok(())
    }

    pub async fn set_engine(&mut self, enabled: bool, skill_level: u8) -> Result<(), String> {
        self.set_engine_full(enabled, skill_level, None, None).await
    }

    pub async fn set_engine_full(
        &mut self,
        enabled: bool,
        skill_level: u8,
        threads: Option<u32>,
        hash_mb: Option<u32>,
    ) -> Result<(), String> {
        self.skill_level = skill_level;
        self.client
            .set_engine(enabled, skill_level as u32, threads, hash_mb)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // --- Internal ---

    /// Apply a snapshot from the server — the single update path.
    fn apply_snapshot(&mut self, snapshot: SessionSnapshot) {
        if let Ok(board) = snapshot.fen.parse::<Board>() {
            self.board = board;
        } else {
            tracing::error!("Failed to parse FEN from server: {}", snapshot.fen);
        }

        // Update mode from snapshot if present
        if let Some(ref gm) = snapshot.game_mode {
            self.mode = game_mode_from_proto(gm);
        }

        // Update pause state from phase
        self.paused = matches!(
            GamePhase::try_from(snapshot.phase).ok(),
            Some(GamePhase::Paused)
        );

        self.snapshot = snapshot;
    }
}

impl Drop for GameSession {
    fn drop(&mut self) {
        // Best effort — can't await in drop
    }
}
