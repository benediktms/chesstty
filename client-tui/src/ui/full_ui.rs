use crate::state::{ClientState, GameMode, InputPhase};
use crate::ui::menu_app;
use crate::ui::widgets::{BoardWidget, ControlsPanel, EngineAnalysisPanel, GameInfoPanel, MoveHistoryPanel, PromotionWidget, UciDebugPanel};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
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
        if let Err(e) = state.set_engine(true, config.skill_level).await {
            state.ui_state.status_message = Some(format!("Failed to enable engine: {}", e));
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
    let mut input_buffer = String::new();

    loop {
        // Poll for events from server (engine thinking, moves, etc.)
        if let Err(e) = state.poll_events().await {
            tracing::warn!("Error polling events: {}", e);
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

        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(50),     // Left: Board
                    Constraint::Length(40),  // Right: Info panels
                ])
                .split(f.area());

            // Dynamic constraints based on which panels are shown
            let mut right_constraints = vec![Constraint::Length(10)]; // Game info

            // Add engine panel if shown
            if state.ui_state.show_engine_panel {
                right_constraints.push(Constraint::Length(12)); // Engine analysis
            }

            // Add move history (takes remaining space if no debug panel)
            if state.ui_state.show_debug_panel {
                right_constraints.push(Constraint::Length(15)); // Move history (fixed)
            } else {
                right_constraints.push(Constraint::Min(15)); // Move history (flexible)
            }

            // Controls panel
            right_constraints.push(Constraint::Length(12)); // Controls

            // UCI debug panel if shown
            if state.ui_state.show_debug_panel {
                right_constraints.push(Constraint::Min(10)); // UCI debug panel
            }

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(right_constraints)
                .split(chunks[1]);

            // Track which chunk index we're at (depends on visible panels)
            let mut chunk_idx = 0;

            // Render board widget with typeahead support
            let board_widget = BoardWidget {
                client_state: state,
                typeahead_squares: &typeahead_squares,
            };
            f.render_widget(board_widget, chunks[0]);

            // Render game info panel
            let game_info = GameInfoPanel {
                client_state: state,
            };
            f.render_widget(game_info, right_chunks[chunk_idx]);
            chunk_idx += 1;

            // Render engine analysis panel if enabled
            if state.ui_state.show_engine_panel {
                let engine_panel = EngineAnalysisPanel::new(
                    state.ui_state.engine_info.as_ref(),
                    state.ui_state.is_engine_thinking,
                );
                f.render_widget(engine_panel, right_chunks[chunk_idx]);
                chunk_idx += 1;
            }

            // Render move history panel
            let history_widget = MoveHistoryPanel {
                history: state.history(),
                scroll: state.ui_state.move_history_scroll,
            };
            f.render_widget(history_widget, right_chunks[chunk_idx]);
            chunk_idx += 1;

            // Render controls panel with input buffer
            let controls_panel = ControlsPanel::new(&input_buffer);
            f.render_widget(controls_panel, right_chunks[chunk_idx]);
            chunk_idx += 1;

            // Render UCI debug panel if enabled
            if state.ui_state.show_debug_panel {
                let uci_panel = UciDebugPanel::new(&state.ui_state.uci_log, state.ui_state.uci_debug_scroll);
                f.render_widget(uci_panel, right_chunks[chunk_idx]);
            }

            // Render promotion dialog if active
            if matches!(state.ui_state.input_phase, InputPhase::SelectPromotion { .. }) {
                let promotion_widget = PromotionWidget {
                    selected_piece: state.ui_state.selected_promotion_piece,
                };
                f.render_widget(promotion_widget, f.area());
            }
        })?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Char('@') => {
                        state.toggle_debug_panel();
                    }
                    KeyCode::Char('#') => {
                        state.toggle_engine_panel();
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            handle_input(state, &input_buffer).await;
                            input_buffer.clear();
                        }
                    }
                    KeyCode::Esc => {
                        state.clear_selection();
                        input_buffer.clear();
                    }
                    KeyCode::PageUp => {
                        // Scroll up in move history
                        state.ui_state.move_history_scroll = state.ui_state.move_history_scroll.saturating_sub(5);
                        if state.ui_state.show_debug_panel {
                            state.ui_state.uci_debug_scroll = state.ui_state.uci_debug_scroll.saturating_sub(5);
                        }
                    }
                    KeyCode::PageDown => {
                        // Scroll down in move history
                        state.ui_state.move_history_scroll = state.ui_state.move_history_scroll.saturating_add(5);
                        if state.ui_state.show_debug_panel {
                            state.ui_state.uci_debug_scroll = state.ui_state.uci_debug_scroll.saturating_add(5);
                        }
                    }
                    KeyCode::Home => {
                        // Scroll to top
                        state.ui_state.move_history_scroll = 0;
                        state.ui_state.uci_debug_scroll = 0;
                    }
                    KeyCode::End => {
                        // Scroll to bottom (set to large value, will be clamped by widget)
                        state.ui_state.move_history_scroll = u16::MAX;
                        state.ui_state.uci_debug_scroll = u16::MAX;
                    }
                    _ => {}
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

async fn handle_input(state: &mut ClientState, input: &str) {
    let input = input.trim().to_lowercase();

    // Check for special commands
    match input.as_str() {
        "undo" | "u" => {
            if let Err(e) = state.undo().await {
                state.ui_state.status_message = Some(format!("Undo error: {}", e));
            }
            return;
        }
        "reset" | "r" => {
            if let Err(e) = state.reset(None).await {
                state.ui_state.status_message = Some(format!("Reset error: {}", e));
            }
            return;
        }
        _ => {}
    }

    // Parse square notation (e.g., "e2", "e4")
    if input.len() == 2 {
        use crate::converters::parse_square;
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
            "Enter a square (e.g., 'e2'). Use 'undo'/'reset' for special commands".to_string(),
        );
    }
}
