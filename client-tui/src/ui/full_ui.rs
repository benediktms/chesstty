use crate::state::{ClientState, GameMode, InputPhase};
use crate::ui::menu_app;
use crate::ui::pane::PaneId;
use crate::ui::widgets::{BoardWidget, EngineAnalysisPanel, GameInfoPanel, MiniBoardWidget, MoveHistoryPanel, PopupMenuWidget, PromotionWidget, UciDebugPanel};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
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

pub async fn run_app() -> anyhow::Result<()> {
    // Show menu and get game configuration
    let config = match menu_app::show_menu().await? {
        Some(cfg) => cfg,
        None => {
            // User quit from menu
            return Ok(());
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Connect to server and create client state
    let mut state = match ClientState::new("http://[::1]:50051").await {
        Ok(state) => state,
        Err(e) => {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
            return Err(anyhow::anyhow!("Failed to connect to server: {}", e));
        }
    };

    // Apply menu configuration
    state.skill_level = config.skill_level;

    // Set starting position if custom FEN provided
    if let Some(fen) = config.start_fen {
        if let Err(e) = state.reset(Some(fen)).await {
            state.ui_state.status_message = Some(format!("Failed to set FEN: {}", e));
        }
    }

    // Enable engine if needed based on game mode
    let needs_engine = matches!(config.mode, GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine);

    // Now assign mode to state
    state.mode = config.mode;

    if needs_engine {
        if let Err(e) = state
            .set_engine_full(true, config.skill_level, config.engine_threads, config.engine_hash_mb)
            .await
        {
            state.ui_state.status_message = Some(format!("Failed to enable engine: {}", e));
        }
    }

    // Initialize timer if time control is set
    if let Some(seconds) = config.time_control_seconds {
        use crate::timer::ChessTimer;
        let timer = ChessTimer::new(std::time::Duration::from_secs(seconds));
        state.timer = Some(timer);
        // Start white's clock immediately
        if let Some(ref mut t) = state.timer {
            t.switch_to(crate::state::PlayerColor::White);
        }
    }

    // Start event stream to receive server events
    if let Err(e) = state.start_event_stream().await {
        state.ui_state.status_message = Some(format!("Failed to start event stream: {}", e));
    }

    let result = run_ui_loop(&mut terminal, &mut state).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_ui_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut ClientState,
) -> anyhow::Result<()> {
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
        // This eliminates the blocking poll — we react instantly to any event.
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

        // Tick the timer
        if let Some(ref mut timer) = state.timer {
            timer.tick();
            for &side in &[crate::state::PlayerColor::White, crate::state::PlayerColor::Black] {
                if timer.is_flag_fallen(side) && timer.active_side() == Some(side) {
                    let side_name = match side {
                        crate::state::PlayerColor::White => "White",
                        crate::state::PlayerColor::Black => "Black",
                    };
                    state.ui_state.status_message = Some(format!("{}'s time has expired!", side_name));
                    timer.pause();
                }
            }
        }

        // Drain any additional buffered server events (non-blocking)
        loop {
            match state.poll_events().await {
                Ok(true) => continue,  // Got an event, try again
                _ => break,            // No more events or error
            }
        }

        // Refresh state from server if needed (after MoveMadeEvent)
        if state.ui_state.needs_refresh {
            state.ui_state.needs_refresh = false;
            if let Err(e) = state.refresh_from_server().await {
                tracing::warn!("Error refreshing state: {}", e);
            }
        }

        // Calculate typeahead squares based on current input
        let typeahead_squares = if !input_buffer.is_empty()
            && matches!(state.ui_state.input_phase, InputPhase::SelectPiece) {
            state.filter_selectable_by_input(&input_buffer)
        } else {
            Vec::new()
        };

        // Snapshot pane state for rendering (avoids borrow conflicts)
        let selected_panel = state.ui_state.focus_stack.selected_pane();
        let expanded_panel = state.ui_state.focus_stack.expanded_pane();
        let show_engine = state.ui_state.pane_manager.is_visible(PaneId::EngineAnalysis);
        let show_debug = state.ui_state.pane_manager.is_visible(PaneId::UciDebug);
        let engine_scroll = state.ui_state.pane_manager.scroll(PaneId::EngineAnalysis);
        let history_scroll = state.ui_state.pane_manager.scroll(PaneId::MoveHistory);
        let debug_scroll = state.ui_state.pane_manager.scroll(PaneId::UciDebug);

        // Draw UI
        terminal.draw(|f| {
            use ratatui::layout::Rect;
            use ratatui::text::{Line, Span};
            use ratatui::widgets::Paragraph;

            // Main vertical split: content area (top) and controls line (bottom)
            let main_vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),     // Content area
                    Constraint::Length(1),   // Controls line at bottom
                ])
                .split(f.area());

            // Content area vertical split: board/panels area and UCI panel (if shown)
            let show_debug_panel = show_debug && expanded_panel != Some(PaneId::UciDebug);
            let mut content_constraints = vec![Constraint::Min(20)]; // Board + panels area
            if show_debug_panel {
                content_constraints.push(Constraint::Length(15)); // UCI panel
            }

            let content_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(content_constraints)
                .split(main_vertical[0]);

            // Split board/panels area horizontally: left (board or expanded pane) and right (info panels)
            let board_area_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(50),     // Left: Board or expanded pane
                    Constraint::Length(40),  // Right: Info panels
                ])
                .split(content_chunks[0]);

            let left_area = board_area_chunks[0];

            // Dynamic constraints for right side panels (exclude expanded pane)
            let mut right_constraints = vec![Constraint::Length(10)]; // Game info (always)

            let show_engine_in_right = show_engine && expanded_panel != Some(PaneId::EngineAnalysis);
            if show_engine_in_right {
                right_constraints.push(Constraint::Length(12)); // Engine analysis
            }

            let show_history_in_right = expanded_panel != Some(PaneId::MoveHistory);
            if show_history_in_right {
                right_constraints.push(Constraint::Min(15)); // Move history
            }

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(right_constraints)
                .split(board_area_chunks[1]);

            // === LEFT AREA: Board or expanded pane ===
            if let Some(exp_pane) = expanded_panel {
                // Render expanded pane in the left area
                match exp_pane {
                    PaneId::MoveHistory => {
                        let widget = MoveHistoryPanel::expanded(
                            state.history(),
                            history_scroll,
                        );
                        f.render_widget(widget, left_area);
                    }
                    PaneId::EngineAnalysis => {
                        let widget = EngineAnalysisPanel::new(
                            state.ui_state.engine_info.as_ref(),
                            state.ui_state.is_engine_thinking,
                            engine_scroll,
                            true,
                        );
                        f.render_widget(widget, left_area);
                    }
                    PaneId::UciDebug => {
                        let widget = UciDebugPanel::new(
                            &state.ui_state.uci_log,
                            debug_scroll,
                            true,
                        );
                        f.render_widget(widget, left_area);
                    }
                    PaneId::GameInfo => {
                        // GameInfo is not expandable, but handle gracefully
                        let widget = GameInfoPanel { client_state: state };
                        f.render_widget(widget, left_area);
                    }
                }

                // Overlay mini-board in bottom-right corner of left area
                let mini_width = 20u16;
                let mini_height = 11u16;
                if left_area.width >= mini_width + 2 && left_area.height >= mini_height + 2 {
                    let mini_area = Rect {
                        x: left_area.x + left_area.width - mini_width,
                        y: left_area.y + left_area.height - mini_height,
                        width: mini_width,
                        height: mini_height,
                    };
                    let is_flipped = matches!(state.mode, GameMode::HumanVsEngine { human_side: crate::state::PlayerColor::Black });
                    let mini_board = MiniBoardWidget {
                        board: state.board(),
                        flipped: is_flipped,
                    };
                    f.render_widget(mini_board, mini_area);
                }
            } else {
                // Normal: render full board
                let is_flipped = matches!(state.mode, GameMode::HumanVsEngine { human_side: crate::state::PlayerColor::Black });
                let board_widget = BoardWidget {
                    client_state: state,
                    typeahead_squares: &typeahead_squares,
                    flipped: is_flipped,
                };
                f.render_widget(board_widget, left_area);
            }

            // === RIGHT PANELS ===
            let mut chunk_idx = 0;

            // Render game info panel (always in right column)
            let game_info = GameInfoPanel {
                client_state: state,
            };
            f.render_widget(game_info, right_chunks[chunk_idx]);
            chunk_idx += 1;

            // Render engine analysis panel if visible and not expanded
            if show_engine_in_right {
                let is_selected = selected_panel == Some(PaneId::EngineAnalysis);
                let engine_panel = EngineAnalysisPanel::new(
                    state.ui_state.engine_info.as_ref(),
                    state.ui_state.is_engine_thinking,
                    engine_scroll,
                    is_selected,
                );
                f.render_widget(engine_panel, right_chunks[chunk_idx]);
                chunk_idx += 1;
            }

            // Render move history panel if not expanded
            if show_history_in_right {
                let is_selected = selected_panel == Some(PaneId::MoveHistory);
                let history_widget = MoveHistoryPanel::new(
                    state.history(),
                    history_scroll,
                    is_selected,
                );
                f.render_widget(history_widget, right_chunks[chunk_idx]);
            }

            // Render UCI debug panel if visible and not expanded
            if show_debug_panel {
                let is_selected = selected_panel == Some(PaneId::UciDebug);
                let uci_panel = UciDebugPanel::new(
                    &state.ui_state.uci_log,
                    debug_scroll,
                    is_selected,
                );
                f.render_widget(uci_panel, content_chunks[1]);
            }

            // Render controls as a single line at the bottom
            let mut controls_spans = vec![];

            if !input_buffer.is_empty() {
                controls_spans.push(Span::styled(
                    format!("Input: {} | ", input_buffer),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            }

            // Game controls
            if state.is_undo_allowed() {
                controls_spans.push(Span::styled("u", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
                controls_spans.push(Span::raw(" Undo | "));
            }
            controls_spans.push(Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Menu | "));

            // Panel controls
            controls_spans.push(Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Panels | "));
            controls_spans.push(Span::styled("\u{2190}\u{2192}", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Select | "));
            controls_spans.push(Span::styled("\u{2191}\u{2193}", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Scroll | "));
            controls_spans.push(Span::styled("@", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" UCI | "));
            controls_spans.push(Span::styled("#", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Engine | "));
            controls_spans.push(Span::styled("Ctrl+C", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
            controls_spans.push(Span::raw(" Quit"));

            let controls_line = Paragraph::new(Line::from(controls_spans))
                .style(Style::default().bg(Color::Black));
            f.render_widget(controls_line, main_vertical[1]);

            // Render promotion dialog if active
            if matches!(state.ui_state.input_phase, InputPhase::SelectPromotion { .. }) {
                let promotion_widget = PromotionWidget {
                    selected_piece: state.ui_state.selected_promotion_piece,
                };
                f.render_widget(promotion_widget, f.area());
            }

            // Render popup menu if active
            if let Some(ref popup_state) = state.ui_state.popup_menu {
                let popup_widget = PopupMenuWidget { state: popup_state };
                f.render_widget(popup_widget, f.area());
            }
        })?;

        // Handle keyboard event if one arrived
        if let Some(Event::Key(key)) = term_event {
            match input::handle_key(state, &mut input_buffer, key).await {
                AppAction::Continue => {}
                AppAction::Quit => break,
                AppAction::ReturnToMenu => break,
                AppAction::SuspendAndReturnToMenu => {
                    let session = crate::session_file::build_saved_session(
                        state.fen(),
                        state.history(),
                        &state.mode,
                        state.skill_level,
                    );
                    if let Err(e) = crate::session_file::save_session(&session) {
                        tracing::error!("Failed to save session: {}", e);
                    }
                    break;
                }
            }
        }

        // Check for engine moves if it's engine's turn
        if state.is_engine_turn() {
            if let Err(e) = state.make_engine_move().await {
                state.ui_state.status_message = Some(format!("Engine error: {}", e));
            }
        }
    }

    Ok(())
}

pub(super) async fn handle_input(state: &mut ClientState, input: &str) {
    let input = input.trim().to_lowercase();

    // Check for special commands
    match input.as_str() {
        "undo" | "u" => {
            if !state.is_undo_allowed() {
                state.ui_state.status_message = Some("Undo is only available in Human vs Engine mode with Beginner difficulty".to_string());
                return;
            }
            if let Err(e) = state.undo().await {
                state.ui_state.status_message = Some(format!("Undo error: {}", e));
            }
            return;
        }
        _ => {}
    }

    // Parse square notation (e.g., "e2", "e4")
    if input.len() == 2 {
        use chess_common::parse_square;
        use cozy_chess::Piece;

        match state.ui_state.input_phase {
            InputPhase::SelectPiece => {
                // First square - select piece
                if let Some(square) = parse_square(&input) {
                    if state.ui_state.selectable_squares.contains(&square) {
                        state.select_square(square);
                    } else {
                        state.ui_state.status_message =
                            Some("No piece on that square or not your turn".to_string());
                    }
                } else {
                    state.ui_state.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectDestination => {
                // Second square - move to destination
                if let Some(square) = parse_square(&input) {
                    if let Err(e) = state.try_move_to(square).await {
                        state.ui_state.status_message = Some(format!("Move error: {}", e));
                    }
                } else {
                    state.ui_state.status_message = Some("Invalid square".to_string());
                }
            }
            InputPhase::SelectPromotion { from, to } => {
                // Promotion piece selection
                let piece = match input.as_str() {
                    "q" | "queen" => Piece::Queen,
                    "r" | "rook" => Piece::Rook,
                    "b" | "bishop" => Piece::Bishop,
                    "n" | "knight" => Piece::Knight,
                    _ => {
                        state.ui_state.status_message = Some(
                            "Invalid promotion piece. Use q/r/b/n for queen/rook/bishop/knight"
                                .to_string(),
                        );
                        return;
                    }
                };

                if let Err(e) = state.execute_promotion(from, to, piece).await {
                    state.ui_state.status_message = Some(format!("Promotion error: {}", e));
                }
            }
        }
    } else {
        state.ui_state.status_message = Some(
            "Enter a square (e.g., 'e2'). Use 'undo' for special commands".to_string(),
        );
    }
}
