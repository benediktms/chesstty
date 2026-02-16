use crate::state::{ClientState, GameMode, InputPhase, PlayerColor};
use crate::ui::menu_app;
use crate::ui::pane::PaneId;
use crate::ui::widgets::{
    BoardWidget, EngineAnalysisPanel, GameInfoPanel, MiniBoardWidget, MoveHistoryPanel,
    PopupMenuWidget, PromotionWidget, UciDebugPanel,
};
use chess_client::{GameModeProto, GameModeType, PlayerSideProto};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    Terminal,
};
use std::io;
use std::time::Duration;

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
}

pub async fn run_app() -> anyhow::Result<()> {
    // Outer loop: menu → game → menu → game → ...
    loop {
        // Pre-fetch data from server for the menu
        let (suspended, positions) =
            match chess_client::ChessClient::connect("http://[::1]:50051").await {
                Ok(mut client) => {
                    let sessions = client.list_suspended_sessions().await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to list suspended sessions: {}", e);
                        vec![]
                    });
                    let positions = client.list_positions().await.unwrap_or_else(|e| {
                        tracing::warn!("Failed to list positions: {}", e);
                        vec![]
                    });
                    (sessions, positions)
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to server: {}", e);
                    (vec![], vec![])
                }
            };

        // Show menu and get game configuration
        let config = match menu_app::show_menu(suspended, positions).await? {
            Some(cfg) => cfg,
            None => return Ok(()), // User quit from menu
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
            Err(e) => return Err(e),
        }
    }
}

/// Set up a game session from config and run the UI loop.
async fn run_game<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: menu_app::GameConfig,
) -> anyhow::Result<ExitReason> {
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
    let mut state = ClientState::new(
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
                    state.ui.status_message = Some(format!("Failed to sync state: {}", e));
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

                // Re-enable engine if needed
                let needs_engine = matches!(
                    state.mode,
                    GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
                );
                if needs_engine {
                    if let Err(e) = state.set_engine(true, state.skill_level).await {
                        state.ui.status_message = Some(format!("Failed to enable engine: {}", e));
                    }
                }

                state.ui.status_message = Some("Session resumed".to_string());
            }
            Err(e) => {
                state.ui.status_message = Some(format!("Failed to resume session: {}", e));
            }
        }
    } else {
        // New game setup — game mode already set on server via create_session
        state.skill_level = config.skill_level;
        state.mode = config.mode;

        let needs_engine = matches!(
            state.mode,
            GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
        );
        if needs_engine {
            // Configuring the engine will trigger maybe_auto_trigger on the server,
            // which will auto-start engine moves for EvE/HvE modes.
            if let Err(e) = state
                .set_engine_full(
                    true,
                    config.skill_level,
                    config.engine_threads,
                    config.engine_hash_mb,
                )
                .await
            {
                state.ui.status_message = Some(format!("Failed to enable engine: {}", e));
            }
        }
    }

    // Start event stream to receive server events
    if let Err(e) = state.start_event_stream().await {
        state.ui.status_message = Some(format!("Failed to start event stream: {}", e));
    }

    run_ui_loop(terminal, &mut state).await
}

async fn run_ui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut ClientState,
) -> anyhow::Result<ExitReason> {
    use super::input::{self, AppAction};
    use crossterm::event::EventStream;
    use futures::StreamExt;

    let mut input_buffer = String::new();
    let mut term_events = EventStream::new();

    // UI refresh interval — controls max frame rate (~30fps).
    // Keyboard and server events wake the loop immediately via select!.
    let mut ui_tick = tokio::time::interval(Duration::from_millis(33));

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
            _ = ui_tick.tick() => {
                None
            }
        };

        // Timer is server-owned — no client-side ticking needed.
        // The server ticks the timer and sends updated snapshots.

        // Drain any additional buffered server events (non-blocking)
        while let Ok(true) = state.poll_events().await {
            continue;
        }

        // Calculate typeahead squares based on current input
        let typeahead_squares = if !input_buffer.is_empty()
            && matches!(state.ui.input_phase, InputPhase::SelectPiece)
        {
            state.filter_selectable_by_input(&input_buffer)
        } else {
            Vec::new()
        };

        // Snapshot pane state for rendering (avoids borrow conflicts)
        let selected_panel = state.ui.focus_stack.selected_pane();
        let expanded_panel = state.ui.focus_stack.expanded_pane();
        let show_engine = state.ui.pane_manager.is_visible(PaneId::EngineAnalysis);
        let show_debug = state.ui.pane_manager.is_visible(PaneId::UciDebug);
        let engine_scroll = state.ui.pane_manager.scroll(PaneId::EngineAnalysis);
        let history_scroll = state.ui.pane_manager.scroll(PaneId::MoveHistory);
        let debug_scroll = state.ui.pane_manager.scroll(PaneId::UciDebug);

        // Draw UI
        terminal.draw(|f| {
            use ratatui::layout::Rect;
            use ratatui::text::{Line, Span};
            use ratatui::widgets::Paragraph;

            let main_vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(1)])
                .split(f.area());

            let show_debug_panel = show_debug && expanded_panel != Some(PaneId::UciDebug);
            let mut content_constraints = vec![Constraint::Min(20)];
            if show_debug_panel {
                content_constraints.push(Constraint::Length(15));
            }

            let content_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(content_constraints)
                .split(main_vertical[0]);

            let board_area_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(50), Constraint::Length(40)])
                .split(content_chunks[0]);

            let left_area = board_area_chunks[0];

            let mut right_constraints = vec![Constraint::Length(10)];

            let show_engine_in_right =
                show_engine && expanded_panel != Some(PaneId::EngineAnalysis);
            if show_engine_in_right {
                right_constraints.push(Constraint::Length(12));
            }

            let show_history_in_right = expanded_panel != Some(PaneId::MoveHistory);
            if show_history_in_right {
                right_constraints.push(Constraint::Min(15));
            }

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(right_constraints)
                .split(board_area_chunks[1]);

            // === LEFT AREA ===
            if let Some(exp_pane) = expanded_panel {
                match exp_pane {
                    PaneId::MoveHistory => {
                        let widget = MoveHistoryPanel::expanded(state.history(), history_scroll);
                        f.render_widget(widget, left_area);
                    }
                    PaneId::EngineAnalysis => {
                        let widget = EngineAnalysisPanel::new(
                            state.ui.engine_info.as_ref(),
                            state.ui.is_engine_thinking,
                            engine_scroll,
                            true,
                        );
                        f.render_widget(widget, left_area);
                    }
                    PaneId::UciDebug => {
                        let widget = UciDebugPanel::new(&state.ui.uci_log, debug_scroll, true);
                        f.render_widget(widget, left_area);
                    }
                    PaneId::GameInfo => {
                        let widget = GameInfoPanel::new(state);
                        f.render_widget(widget, left_area);
                    }
                }

                let mini_width = 20u16;
                let mini_height = 11u16;
                if left_area.width >= mini_width + 2 && left_area.height >= mini_height + 2 {
                    let mini_area = Rect {
                        x: left_area.x + left_area.width - mini_width,
                        y: left_area.y + left_area.height - mini_height,
                        width: mini_width,
                        height: mini_height,
                    };
                    let is_flipped = matches!(
                        state.mode,
                        GameMode::HumanVsEngine {
                            human_side: PlayerColor::Black
                        }
                    );
                    let mini_board = MiniBoardWidget {
                        board: state.board(),
                        flipped: is_flipped,
                    };
                    f.render_widget(mini_board, mini_area);
                }
            } else {
                let is_flipped = matches!(
                    state.mode,
                    GameMode::HumanVsEngine {
                        human_side: PlayerColor::Black
                    }
                );
                let board_widget = BoardWidget {
                    client_state: state,
                    typeahead_squares: &typeahead_squares,
                    flipped: is_flipped,
                };
                f.render_widget(board_widget, left_area);
            }

            // === RIGHT PANELS ===
            let mut chunk_idx = 0;

            let game_info = GameInfoPanel {
                client_state: state,
            };
            f.render_widget(game_info, right_chunks[chunk_idx]);
            chunk_idx += 1;

            if show_engine_in_right {
                let is_selected = selected_panel == Some(PaneId::EngineAnalysis);
                let engine_panel = EngineAnalysisPanel::new(
                    state.ui.engine_info.as_ref(),
                    state.ui.is_engine_thinking,
                    engine_scroll,
                    is_selected,
                );
                f.render_widget(engine_panel, right_chunks[chunk_idx]);
                chunk_idx += 1;
            }

            if show_history_in_right {
                let is_selected = selected_panel == Some(PaneId::MoveHistory);
                let history_widget =
                    MoveHistoryPanel::new(state.history(), history_scroll, is_selected);
                f.render_widget(history_widget, right_chunks[chunk_idx]);
            }

            if show_debug_panel {
                let is_selected = selected_panel == Some(PaneId::UciDebug);
                let uci_panel = UciDebugPanel::new(&state.ui.uci_log, debug_scroll, is_selected);
                f.render_widget(uci_panel, content_chunks[1]);
            }

            // Controls line
            let mut controls_spans = vec![];

            if !input_buffer.is_empty() {
                controls_spans.push(Span::styled(
                    format!("Input: {} | ", input_buffer),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            // Pause indicator
            if matches!(
                state.mode,
                GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
            ) {
                if state.ui.paused {
                    controls_spans.push(Span::styled(
                        "PAUSED",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ));
                    controls_spans.push(Span::raw(" | "));
                }
                controls_spans.push(Span::styled(
                    "p",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                controls_spans.push(Span::raw(" Pause | "));
            }

            if state.is_undo_allowed() {
                controls_spans.push(Span::styled(
                    "u",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                controls_spans.push(Span::raw(" Undo | "));
            }
            controls_spans.push(Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Menu | "));
            controls_spans.push(Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Panels | "));
            controls_spans.push(Span::styled(
                "\u{2190}\u{2192}",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Select | "));
            controls_spans.push(Span::styled(
                "\u{2191}\u{2193}",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Scroll | "));
            controls_spans.push(Span::styled(
                "@",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" UCI | "));
            controls_spans.push(Span::styled(
                "#",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Engine | "));
            controls_spans.push(Span::styled(
                "Ctrl+C",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
            controls_spans.push(Span::raw(" Quit"));

            let controls_line =
                Paragraph::new(Line::from(controls_spans)).style(Style::default().bg(Color::Black));
            f.render_widget(controls_line, main_vertical[1]);

            // Overlays
            if matches!(state.ui.input_phase, InputPhase::SelectPromotion { .. }) {
                let promotion_widget = PromotionWidget {
                    selected_piece: state.ui.selected_promotion_piece,
                };
                f.render_widget(promotion_widget, f.area());
            }

            if let Some(ref popup_state) = state.ui.popup_menu {
                let popup_widget = PopupMenuWidget { state: popup_state };
                f.render_widget(popup_widget, f.area());
            }
        })?;

        // Handle keyboard event if one arrived
        if let Some(Event::Key(key)) = term_event {
            match input::handle_key(state, &mut input_buffer, key).await {
                AppAction::Continue => {}
                AppAction::Quit => return Ok(ExitReason::Quit),
                AppAction::ReturnToMenu => return Ok(ExitReason::ReturnToMenu),
                AppAction::SuspendAndReturnToMenu => {
                    // Suspend via server RPC (server stores all session metadata)
                    if let Err(e) = state.client.suspend_session().await {
                        tracing::error!("Failed to suspend session: {}", e);
                    }
                    return Ok(ExitReason::ReturnToMenu);
                }
            }
        }
    }
}

pub(super) async fn handle_input(state: &mut ClientState, input: &str) {
    let input = input.trim().to_lowercase();

    // Check for special commands
    match input.as_str() {
        "undo" | "u" => {
            if !state.is_undo_allowed() {
                state.ui.status_message = Some(
                    "Undo is only available in Human vs Engine mode with Beginner difficulty"
                        .to_string(),
                );
                return;
            }
            if let Err(e) = state.undo().await {
                state.ui.status_message = Some(format!("Undo error: {}", e));
            }
            return;
        }
        _ => {}
    }

    // Parse square notation (e.g., "e2", "e4")
    if input.len() == 2 {
        use chess::parse_square;
        use cozy_chess::Piece;

        match state.ui.input_phase {
            InputPhase::SelectPiece => {
                if let Some(square) = parse_square(&input) {
                    if state.ui.selectable_squares.contains(&square) {
                        state.select_square(square);
                    } else {
                        state.ui.status_message =
                            Some("No piece on that square or not your turn".to_string());
                    }
                } else {
                    state.ui.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectDestination => {
                if let Some(square) = parse_square(&input) {
                    if let Err(e) = state.try_move_to(square).await {
                        state.ui.status_message = Some(format!("Move error: {}", e));
                    }
                } else {
                    state.ui.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectPromotion { from, to } => {
                let piece = match input.as_str() {
                    "q" | "queen" => Piece::Queen,
                    "r" | "rook" => Piece::Rook,
                    "b" | "bishop" => Piece::Bishop,
                    "n" | "knight" => Piece::Knight,
                    _ => {
                        state.ui.status_message = Some(
                            "Invalid promotion piece. Use q/r/b/n for queen/rook/bishop/knight"
                                .to_string(),
                        );
                        return;
                    }
                };

                if let Err(e) = state.execute_promotion(from, to, piece).await {
                    state.ui.status_message = Some(format!("Promotion error: {}", e));
                }
            }
        }
    } else {
        state.ui.status_message =
            Some("Enter a square (e.g., 'e2'). Use 'undo' for special commands".to_string());
    }
}
