use crate::state::{GameMode, GameSession, PlayerColor};
use crate::ui::fsm::render_spec::InputPhase;
use crate::ui::menu_app;
use chess_client::{GameModeProto, GameModeType, PlayerSideProto};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Layout, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// Get the socket path for server communication.
///
/// Priority:
/// 1. CHESSTTY_SOCKET_PATH env variable if set
/// 2. /tmp/chesstty.sock as fallback
fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from("/tmp/chesstty.sock")
}

/// Convert client-side GameMode to proto representation.
fn game_mode_to_proto(mode: &GameMode) -> GameModeProto {
    match mode {
        GameMode::HumanVsHuman => GameModeProto {
            mode: GameModeType::HumanVsHuman as i32,
            human_side: None,
        },
        GameMode::HumanVsEngine { human_side } => GameModeProto {
            mode: GameModeType::HumanVsEngine as i32,
            human_side: Some(match human_side {
                PlayerColor::White => PlayerSideProto::White as i32,
                PlayerColor::Black => PlayerSideProto::Black as i32,
            }),
        },
        GameMode::EngineVsEngine => GameModeProto {
            mode: GameModeType::EngineVsEngine as i32,
            human_side: None,
        },
        GameMode::AnalysisMode => GameModeProto {
            mode: GameModeType::Analysis as i32,
            human_side: None,
        },
        GameMode::ReviewMode => GameModeProto {
            mode: GameModeType::Review as i32,
            human_side: None,
        },
    }
}

/// Why the game loop exited.
enum ExitReason {
    Quit,
    ReturnToMenu,
    PlaySnapshot(Box<menu_app::GameConfig>),
}

pub async fn run_app() -> anyhow::Result<()> {
    // Outer loop: menu → game → menu → game → ...
    loop {
        // Pre-fetch data from server for the menu
        let (suspended, positions, finished_games) =
            match chess_client::ChessClient::connect_uds(&get_socket_path()).await {
                Ok(mut client) => {
                    let sessions = client.list_suspended_sessions().await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to list suspended sessions: {}", e);
                        vec![]
                    });
                    let positions = client.list_positions().await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to list positions: {}", e);
                        vec![]
                    });
                    let finished = client.list_finished_games().await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to list finished games: {}", e);
                        vec![]
                    });
                    (sessions, positions, finished)
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to server: {}", e);
                    (vec![], vec![], vec![])
                }
            };

        // Show menu and get game configuration
        let menu_action = menu_app::show_menu(suspended, positions, finished_games).await?;

        let config = match menu_action {
            menu_app::MenuAction::Quit => return Ok(()),
            menu_app::MenuAction::EnqueueReview(game_id) => {
                // Enqueue analysis and return to menu
                if let Ok(mut client) =
                    chess_client::ChessClient::connect_uds(&get_socket_path()).await
                {
                    match client.enqueue_review(&game_id).await {
                        Ok(_) => tracing::info!(game_id = %game_id, "Review enqueued"),
                        Err(e) => tracing::warn!(game_id = %game_id, "Failed to enqueue: {}", e),
                    }
                }
                continue;
            }
            menu_app::MenuAction::StartGame(mut cfg) => {
                // If review mode, fetch the review data before entering game
                if cfg.mode == crate::state::GameMode::ReviewMode {
                    if let Some(ref game_id) = cfg.resume_session_id {
                        tracing::info!(game_id = %game_id, "Fetching review data");
                        match chess_client::ChessClient::connect_uds(&get_socket_path()).await {
                            Ok(mut client) => match client.get_game_review(game_id).await {
                                Ok(review) => {
                                    tracing::info!(
                                        game_id = %game_id,
                                        plies = review.total_plies,
                                        "Review fetched, entering review mode"
                                    );
                                    cfg.review_data = Some(review);

                                    // Also try to fetch advanced analysis
                                    match client.get_advanced_analysis(game_id).await {
                                        Ok(advanced) => {
                                            tracing::info!(
                                                game_id = %game_id,
                                                "Advanced analysis fetched"
                                            );
                                            cfg.advanced_data = Some(advanced);
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                game_id = %game_id,
                                                "Advanced analysis not available: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(game_id = %game_id, "Failed to fetch review: {}", e);
                                    continue;
                                }
                            },
                            Err(e) => {
                                tracing::error!("Failed to connect to server for review: {}", e);
                                continue;
                            }
                        }
                    } else {
                        tracing::error!("Review mode but no game_id");
                        continue;
                    }
                }
                *cfg
            }
        };

        // Setup terminal for game
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = run_game(&mut terminal, config).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        match result {
            Ok(ExitReason::Quit) => return Ok(()),
            Ok(ExitReason::ReturnToMenu) => continue, // Loop back to menu
            Ok(ExitReason::PlaySnapshot(config)) => {
                // Re-enter game directly with the snapshot config (skip menu)
                enable_raw_mode()?;
                let mut stdout = io::stdout();
                execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
                let backend = CrosstermBackend::new(stdout);
                let mut terminal = Terminal::new(backend)?;

                let result = run_game(&mut terminal, *config).await;

                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                match result {
                    Ok(ExitReason::Quit) => return Ok(()),
                    Ok(ExitReason::ReturnToMenu) => continue,
                    Ok(ExitReason::PlaySnapshot(_inner_config)) => {
                        // Nested snapshot — not expected but handle gracefully
                        tracing::warn!("Nested PlaySnapshot, returning to menu");
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }
}

/// Set up a game session from config and run the UI loop.
async fn run_game<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: menu_app::GameConfig,
) -> anyhow::Result<ExitReason> {
    // Create FSM
    use crate::ui::fsm::{UiMode, UiStateMachine};
    let mut fsm = UiStateMachine::default();

    // Review mode: no server session, just local navigation
    if config.mode == GameMode::ReviewMode {
        if let Some(review_data) = config.review_data {
            let mut state = GameSession::new_review(
                "http://[::1]:50051",
                review_data,
                config.review_game_mode,
                config.review_skill_level.unwrap_or(0),
                config.advanced_data,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;
            // Transition FSM to review board
            fsm.transition_to(UiMode::ReviewBoard);
            return run_ui_loop(terminal, &mut state, fsm).await;
        }
        return Ok(ExitReason::ReturnToMenu);
    }

    // Convert game mode to proto for the server
    let game_mode_proto = game_mode_to_proto(&config.mode);

    // Convert timer config to proto (server owns all timer state)
    let timer_proto = config.time_control_seconds.map(|seconds| {
        let ms = seconds * 1000;
        chess_client::TimerState {
            white_remaining_ms: ms,
            black_remaining_ms: ms,
            active_side: None,
        }
    });

    // Connect to server and create client state with game mode, FEN, and timer
    let mut state = GameSession::new(
        "http://[::1]:50051",
        config.start_fen.clone(),
        Some(game_mode_proto),
        timer_proto,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to connect to server: {}", e))?;

    // Handle resume vs new game
    if let Some(ref suspended_id) = config.resume_session_id {
        // Resume a suspended session from the server.
        // The server already knows the game mode and engine config.
        match state.client.resume_suspended_session(suspended_id).await {
            Ok(_snapshot) => {
                if let Err(e) = state.refresh_from_server().await {
                    state.status_message = Some(format!("Failed to sync state: {}", e));
                }

                // Restore local game mode from config metadata (for UI rendering)
                let game_mode_type = config
                    .resume_game_mode
                    .and_then(|v| GameModeType::try_from(v).ok())
                    .unwrap_or(GameModeType::HumanVsHuman);
                state.mode = match game_mode_type {
                    GameModeType::HumanVsHuman => GameMode::HumanVsHuman,
                    GameModeType::HumanVsEngine => {
                        let side = match config
                            .resume_human_side
                            .and_then(|v| PlayerSideProto::try_from(v).ok())
                        {
                            Some(PlayerSideProto::Black) => PlayerColor::Black,
                            _ => PlayerColor::White,
                        };
                        GameMode::HumanVsEngine { human_side: side }
                    }
                    GameModeType::EngineVsEngine => GameMode::EngineVsEngine,
                    GameModeType::Analysis => GameMode::AnalysisMode,
                    GameModeType::Review => GameMode::ReviewMode,
                };

                state.skill_level = config.resume_skill_level.unwrap_or(10);

                state.status_message = Some("Session resumed".to_string());
            }
            Err(e) => {
                state.status_message = Some(format!("Failed to resume session: {}", e));
            }
        }
    } else {
        // New game setup — game mode already set on server via create_session
        state.skill_level = config.skill_level;
        state.mode = config.mode.clone();
    }

    // Apply pre-history if starting from a snapshot.
    let is_snapshot = config.pre_history.is_some();
    if let Some(pre_history) = config.pre_history {
        state.pre_history = pre_history;
    }

    // Start event stream BEFORE engine config so we don't miss auto-triggered moves
    // (e.g., when it's the engine's turn at the snapshot position)
    if let Err(e) = state.start_event_stream().await {
        state.status_message = Some(format!("Failed to start event stream: {}", e));
    }

    // Configure engine after event stream is active so we don't miss
    // auto-triggered moves (e.g., when it's the engine's turn at a snapshot position)
    let needs_engine = matches!(
        state.mode,
        GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
    );

    // Snapshot games start paused so the player can orient themselves before
    // the engine fires. The user presses 'p' to begin.
    if is_snapshot && needs_engine {
        let _ = state.client.pause().await;
        state.paused = true;
        state.status_message = Some("Paused \u{2014} press p to start".to_string());
    }

    if needs_engine {
        if config.resume_session_id.is_some() {
            // Resume: re-enable engine with stored skill level
            if let Err(e) = state.set_engine(true, state.skill_level).await {
                state.status_message = Some(format!("Failed to enable engine: {}", e));
            }
        } else {
            // New game: full engine configuration
            if let Err(e) = state
                .set_engine_full(
                    true,
                    config.skill_level,
                    config.engine_threads,
                    config.engine_hash_mb,
                )
                .await
            {
                state.status_message = Some(format!("Failed to enable engine: {}", e));
            }
        }
    }

    // Transition FSM to game board
    fsm.transition_to(UiMode::GameBoard);

    run_ui_loop(terminal, &mut state, fsm).await
}

async fn run_ui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut GameSession,
    mut fsm: crate::ui::fsm::UiStateMachine,
) -> anyhow::Result<ExitReason> {
    use super::input::{self, AppAction};
    use crossterm::event::EventStream;
    use futures::StreamExt;

    let mut input_buffer = String::new();
    let mut term_events = EventStream::new();

    // UI refresh interval — controls max frame rate (~30fps).
    // Keyboard and server events wake the loop immediately via select!.
    let mut render_state_tick = tokio::time::interval(Duration::from_millis(33));

    // Auto-play tracking for review mode
    let mut last_auto_advance = std::time::Instant::now();

    loop {
        // Wait for whichever comes first: keyboard, server event, or UI tick.
        let term_event = tokio::select! {
            biased;

            // Keyboard / terminal event (highest priority)
            maybe_event = term_events.next() => {
                match maybe_event {
                    Some(Ok(ev)) => Some(ev),
                    Some(Err(e)) => {
                        tracing::warn!("Terminal event error: {}", e);
                        None
                    }
                    None => None,
                }
            }

            // Server event from gRPC stream
            consumed = async {
                state.poll_event_async().await
            } => {
                if let Err(e) = consumed {
                    tracing::warn!("Error polling server events: {}", e);
                }
                None
            }

            // Periodic UI refresh (timer display, animations)
            _ = render_state_tick.tick() => {
                None
            }
        };

        // Auto-play: advance review ply every 750ms when active
        if let Some(ref mut review) = state.review_state {
            if review.auto_play && last_auto_advance.elapsed() >= Duration::from_millis(750) {
                if review.current_ply >= review.review.total_plies {
                    review.auto_play = false;
                } else {
                    review.next_ply();
                    last_auto_advance = std::time::Instant::now();
                }
            }
        }

        // Timer is server-owned — no client-side ticking needed.
        // The server ticks the timer and sends updated snapshots.

        // Drain any additional buffered server events (non-blocking)
        while let Ok(true) = state.poll_events().await {
            continue;
        }

        // Calculate typeahead squares based on current input and store on FSM
        fsm.typeahead_squares = if fsm.tab_input.active
            && fsm.tab_input.current_tab == 0
            && !fsm.tab_input.typeahead_buffer.is_empty()
        {
            state.filter_selectable_by_input(&fsm.tab_input.typeahead_buffer)
        } else if !input_buffer.is_empty() && matches!(fsm.input_phase, InputPhase::SelectPiece) {
            state.filter_selectable_by_input(&input_buffer)
        } else {
            Vec::new()
        };

        // Snapshot pane state for rendering (avoids borrow conflicts)
        let is_review_mode = matches!(state.mode, GameMode::ReviewMode);

        // Draw UI using FSM-based renderer
        terminal.draw(|f| {
            use crate::ui::fsm::renderer::Renderer;
            // Get layout from FSM
            let layout = fsm.layout(state);
            Renderer::render(f, f.area(), &layout, state, &fsm);
        });

        // Handle keyboard event if one arrived
        if let Some(Event::Key(key)) = term_event {
            match input::handle_key(state, &mut fsm, &mut input_buffer, key).await {
                AppAction::Continue => {}
                AppAction::Quit => {
                    // Review mode has no server session to close
                    if state.review_state.is_none() {
                        if let Err(e) = state.client.close_session().await {
                            tracing::warn!("Failed to close session on qrender_statet: {}", e);
                        }
                    }
                    return Ok(ExitReason::Quit);
                }
                AppAction::ReturnToMenu => {
                    if state.review_state.is_none() {
                        if let Err(e) = state.client.close_session().await {
                            tracing::warn!("Failed to close session on return to menu: {}", e);
                        }
                    }
                    return Ok(ExitReason::ReturnToMenu);
                }
                AppAction::SuspendAndReturnToMenu => {
                    // Suspend via server RPC (server stores all session metadata)
                    if let Err(e) = state.client.suspend_session().await {
                        tracing::error!("Failed to suspend session: {}", e);
                    }
                    return Ok(ExitReason::ReturnToMenu);
                }
                AppAction::PlaySnapshot(config) => {
                    return Ok(ExitReason::PlaySnapshot(config));
                }
            }
        }
    }
}

pub(super) async fn handle_input(
    state: &mut GameSession,
    fsm: &mut crate::ui::fsm::UiStateMachine,
    input: &str,
) {
    let input = input.trim().to_lowercase();

    // Check for special commands
    match input.as_str() {
        "undo" | "u" => {
            if !state.is_undo_allowed() {
                state.status_message = Some(
                    "Undo is only available in Human vs Engine mode with Beginner difficulty"
                        .to_string(),
                );
                return;
            }
            if let Err(e) = state.undo().await {
                state.status_message = Some(format!("Undo error: {}", e));
            }
            return;
        }
        _ => {}
    }

    // Parse square notation (e.g., "e2", "e4")
    if input.len() == 2 {
        use chess::parse_square;
        use cozy_chess::Piece;

        match fsm.input_phase {
            InputPhase::SelectPiece => {
                if let Some(square) = parse_square(&input) {
                    if state.selectable_squares.contains(&square) {
                        state.select_square(square);
                    } else {
                        state.status_message =
                            Some("No piece on that square or not your turn".to_string());
                    }
                } else {
                    state.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectDestination => {
                if let Some(square) = parse_square(&input) {
                    if let Err(e) = state.try_move_to(square).await {
                        state.status_message = Some(format!("Move error: {}", e));
                    }
                } else {
                    state.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectPromotion { from, to } => {
                let piece = match input.as_str() {
                    "q" | "queen" => Piece::Queen,
                    "r" | "rook" => Piece::Rook,
                    "b" | "bishop" => Piece::Bishop,
                    "n" | "knight" => Piece::Knight,
                    _ => {
                        state.status_message = Some(
                            "Invalid promotion piece. Use q/r/b/n for queen/rook/bishop/knight"
                                .to_string(),
                        );
                        return;
                    }
                };

                if let Err(e) = state.execute_promotion(from, to, piece).await {
                    state.status_message = Some(format!("Promotion error: {}", e));
                }
            }
        }
    } else {
        state.status_message =
            Some("Enter a square (e.g., 'e2'). Use 'undo' for special commands".to_string());
    }
}
