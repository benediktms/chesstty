use crate::app::{AppState, FenHistory, GameMode, InputBuffer, InputPhase};
use crate::chess::Game;
use crate::ui::format::format_square_display;
use crate::ui::widgets::{
    BoardWidget, ControlsPanel, FenDialogFocus, FenDialogState, FenDialogWidget, GameInfoPanel,
    MenuState, MenuWidget, MoveHistoryPanel, PromotionWidget, UciDebugPanel,
};
use crate::ui::widgets::menu::StartPositionOption;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;

enum AppMode {
    Menu,
    Game,
}

/// Run the TUI application
pub async fn run_app() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create menu state with loaded FEN history
    let fen_history = FenHistory::load_from_file().unwrap_or_else(|e| {
        eprintln!("Could not load FEN history: {}, using defaults", e);
        FenHistory::new()
    });

    let mut menu_state = MenuState {
        selected_index: 0,
        game_mode: crate::ui::widgets::menu::GameModeOption::HumanVsEngine,
        difficulty: crate::ui::widgets::menu::DifficultyOption::Intermediate,
        time_control: crate::ui::widgets::menu::TimeControlOption::None,
        start_position: StartPositionOption::Standard,
        fen_dialog_state: None,
        fen_history,
        selected_fen: None,
    };
    let mut app_mode = AppMode::Menu;
    let mut app_state = AppState::new();

    // Run main loop
    let result = loop {
        match app_mode {
            AppMode::Menu => {
                terminal.draw(|f| {
                    let menu_widget = MenuWidget::new(&menu_state);
                    f.render_widget(menu_widget, f.area());

                    // Draw FEN dialog on top if open
                    if let Some(dialog_state) = &menu_state.fen_dialog_state {
                        let fen_dialog = FenDialogWidget::new(dialog_state, &menu_state.fen_history);
                        f.render_widget(fen_dialog, f.area());
                    }
                })?;

                // Handle menu input
                if event::poll(std::time::Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        // Handle FEN dialog input first if open
                        if menu_state.fen_dialog_state.is_some() {
                            let was_open = true;
                            handle_fen_dialog_input(key.code, &mut menu_state)?;
                            // If dialog was closed and FEN was selected, start the game
                            if was_open && menu_state.fen_dialog_state.is_none() && menu_state.selected_fen.is_some() {
                                app_state = create_game_from_menu(&menu_state);
                                app_mode = AppMode::Game;
                            }
                            continue;
                        }

                        match key.code {
                            KeyCode::Char('q') => break Ok(()),
                            KeyCode::Up | KeyCode::Char('k') => menu_state.move_up(),
                            KeyCode::Down | KeyCode::Char('j') => {
                                menu_state.move_down(menu_state.items().len())
                            }
                            KeyCode::Left | KeyCode::Char('h') => match menu_state.selected_index {
                                0 => menu_state.cycle_game_mode(),
                                1 => menu_state.cycle_difficulty(),
                                2 => menu_state.cycle_time_control(),
                                3 => menu_state.cycle_start_position(),
                                _ => {}
                            },
                            KeyCode::Right | KeyCode::Char('l') => {
                                match menu_state.selected_index {
                                    0 => menu_state.cycle_game_mode(),
                                    1 => menu_state.cycle_difficulty(),
                                    2 => menu_state.cycle_time_control(),
                                    3 => menu_state.cycle_start_position(),
                                    _ => {}
                                }
                            }
                            KeyCode::Enter => {
                                match menu_state.selected_index {
                                    3 => {
                                        // StartPosition - toggle
                                        menu_state.cycle_start_position();
                                    }
                                    4 => {
                                        // Start Game
                                        if menu_state.start_position == StartPositionOption::CustomFen {
                                            // Open FEN dialog
                                            menu_state.fen_dialog_state = Some(FenDialogState::new());
                                        } else {
                                            // Start game with standard position
                                            menu_state.selected_fen = None;
                                            app_state = create_game_from_menu(&menu_state);
                                            app_mode = AppMode::Game;
                                        }
                                    }
                                    5 => {
                                        // Quit
                                        break Ok(());
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            AppMode::Game => {
                let result = run_game_loop(&mut terminal, &mut app_state).await;
                match result {
                    Ok(true) => {
                        // Return to menu
                        app_mode = AppMode::Menu;
                        menu_state = MenuState::default();
                    }
                    Ok(false) => break Ok(()), // Quit
                    Err(e) => break Err(e),
                }
            }
        }
    };

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

fn create_game_from_menu(menu_state: &MenuState) -> AppState {
    let mode = match menu_state.game_mode {
        crate::ui::widgets::menu::GameModeOption::HumanVsHuman => GameMode::HumanVsHuman,
        crate::ui::widgets::menu::GameModeOption::HumanVsEngine => GameMode::HumanVsEngine {
            human_side: cozy_chess::Color::White,
        },
        crate::ui::widgets::menu::GameModeOption::EngineVsEngine => GameMode::EngineVsEngine,
    };

    // Create game from FEN if selected, otherwise use standard position
    let game = if let Some(fen) = &menu_state.selected_fen {
        match Game::from_fen(fen) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("FEN validation error: {}, using standard position", e);
                Game::new()
            }
        }
    } else {
        Game::new()
    };

    let mut app_state = AppState {
        game,
        mode,
        engine: None,
        skill_level: menu_state.difficulty.skill_level(),
        ui_state: crate::app::state::UiState {
            selected_square: None,
            highlighted_squares: Vec::new(),
            selectable_squares: Vec::new(),
            last_move: None,
            engine_info: None,
            status_message: None,
            input_phase: InputPhase::SelectPiece,
            show_debug_panel: false,
            uci_log: Vec::new(),
            selected_promotion_piece: cozy_chess::Piece::Queen,
        },
    };

    app_state.update_selectable_squares();
    app_state
}

fn handle_fen_dialog_input(
    key: KeyCode,
    menu_state: &mut MenuState,
) -> anyhow::Result<()> {
    if let Some(dialog_state) = &mut menu_state.fen_dialog_state {
        match dialog_state.focus {
            FenDialogFocus::Input => {
                match key {
                    KeyCode::Char(c) => {
                        dialog_state.input_buffer.push(c);
                        dialog_state.validation_error = None;
                    }
                    KeyCode::Backspace => {
                        dialog_state.input_buffer.pop();
                        dialog_state.validation_error = None;
                    }
                    KeyCode::Tab | KeyCode::Char('l') => {
                        dialog_state.focus = FenDialogFocus::HistoryList;
                    }
                    KeyCode::Enter => {
                        // Validate FEN
                        if dialog_state.input_buffer.is_empty() {
                            dialog_state.validation_error = Some("FEN string is empty".to_string());
                        } else {
                            match Game::from_fen(&dialog_state.input_buffer) {
                                Ok(_) => {
                                    // Valid FEN - add to history, save, set selected_fen, close dialog
                                    menu_state.fen_history.add_fen(dialog_state.input_buffer.clone());
                                    if let Err(e) = menu_state.fen_history.save_to_file() {
                                        eprintln!("Failed to save FEN history: {}", e);
                                    }
                                    menu_state.selected_fen = Some(dialog_state.input_buffer.clone());
                                    menu_state.fen_dialog_state = None;
                                }
                                Err(e) => {
                                    dialog_state.validation_error = Some(format!("Invalid FEN: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Esc => {
                        menu_state.fen_dialog_state = None;
                    }
                    _ => {}
                }
            }
            FenDialogFocus::HistoryList => {
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if dialog_state.selected_history_index > 0 {
                            dialog_state.selected_history_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max_index = menu_state.fen_history.entries().len().saturating_sub(1);
                        if dialog_state.selected_history_index < max_index {
                            dialog_state.selected_history_index += 1;
                        }
                    }
                    KeyCode::Tab | KeyCode::Char('h') => {
                        dialog_state.focus = FenDialogFocus::Input;
                    }
                    KeyCode::Enter => {
                        // Use selected FEN from history
                        if let Some(entry) = menu_state.fen_history.entries().get(dialog_state.selected_history_index) {
                            menu_state.selected_fen = Some(entry.fen.clone());
                            menu_state.fen_dialog_state = None;
                        }
                    }
                    KeyCode::Esc => {
                        menu_state.fen_dialog_state = None;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

async fn run_game_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app_state: &mut AppState,
) -> anyhow::Result<bool> {
    let mut input_buffer = InputBuffer::new();
    let mut typeahead_squares: Vec<cozy_chess::Square> = Vec::new();

    // Initialize engine if needed
    if matches!(
        app_state.mode,
        GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
    ) {
        tracing::info!("Initializing engine for game mode: {:?}", app_state.mode);
        match crate::engine::StockfishEngine::spawn(Some(app_state.skill_level)).await {
            Ok(engine) => {
                tracing::info!("Engine spawned successfully");
                app_state.engine = Some(engine);
                app_state.ui_state.status_message = Some("Engine ready!".to_string());

                // Log UCI initialization sequence
                app_state.log_uci_message(
                    crate::app::UciDirection::ToEngine,
                    "uci".to_string(),
                    None,
                );
                app_state.log_uci_message(
                    crate::app::UciDirection::FromEngine,
                    "uciok".to_string(),
                    None,
                );
                app_state.log_uci_message(
                    crate::app::UciDirection::ToEngine,
                    format!("setoption name Skill Level value {}", app_state.skill_level),
                    None,
                );
            }
            Err(e) => {
                tracing::error!("Failed to spawn engine: {}", e);
                app_state.ui_state.status_message = Some(format!("Engine error: {}", e));
            }
        }

        // If engine plays first (black vs engine as white), trigger engine move
        if app_state.is_engine_turn() {
            tracing::info!("Engine plays first, triggering initial engine move");
            if let Err(e) = app_state.make_engine_move().await {
                tracing::error!("Failed to make initial engine move: {}", e);
            }
        } else {
            tracing::info!("Human plays first");
        }
    }

    loop {
        // Check for engine events and apply moves
        if let Some(engine_move) = app_state.process_engine_events() {
            tracing::info!("Processing engine move: from={:?}, to={:?}, promotion={:?}",
                engine_move.from, engine_move.to, engine_move.promotion);

            match app_state.game.make_move(engine_move) {
                Ok(()) => {
                    tracing::info!("Engine move applied successfully");
                    app_state.ui_state.last_move = Some((engine_move.from, engine_move.to));
                    app_state.ui_state.status_message = Some(format!(
                        "Engine played: {} to {}",
                        crate::ui::format::format_square_display(engine_move.from),
                        crate::ui::format::format_square_display(engine_move.to)
                    ));
                    app_state.update_selectable_squares();

                    // Check if engine should move again (engine vs engine)
                    if app_state.is_engine_turn() {
                        tracing::info!("Engine should move again (engine vs engine mode)");
                        if let Err(e) = app_state.make_engine_move().await {
                            tracing::error!("Failed to trigger next engine move: {}", e);
                        }
                    } else {
                        tracing::debug!("Not engine's turn after move");
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to apply engine move: {:?}", e);
                }
            }
        }
        // Update typeahead highlighting based on current input
        if app_state.ui_state.input_phase == InputPhase::SelectPiece {
            typeahead_squares = app_state.filter_selectable_by_input(input_buffer.as_str());
        } else {
            typeahead_squares.clear();
        }

        // Draw UI
        terminal.draw(|f| {
            let area = f.area();

            // Check if terminal is too small
            let min_dimensions = BoardWidget::min_dimensions();
            let min_width = min_dimensions.0; // Just the board width
            let min_height = min_dimensions.1 + 3; // Board + input boxes

            if area.width < min_width || area.height < min_height {
                // Terminal too small - show error message
                use ratatui::widgets::Paragraph;
                let msg = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Terminal too small!",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("Current: {}x{}", area.width, area.height)),
                    Line::from(format!("Minimum: {}x{}", min_width, min_height)),
                    Line::from(""),
                    Line::from("Please resize your terminal."),
                ])
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
                f.render_widget(msg, area);
                return;
            }

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(3)])
                .split(area);

            // Decide layout direction based on available width
            // Calculate minimum for horizontal: small board (76) + panels (35) + margin (5)
            let min_horizontal_width = 116;
            let use_vertical_layout = area.width < min_horizontal_width;

            let (board_area, panels_area) = if use_vertical_layout {
                // Vertical layout: board on top (larger), panels below (smaller)
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(main_chunks[0]);
                (chunks[0], chunks[1])
            } else {
                // Horizontal layout: board on left, panels on right
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(main_chunks[0]);
                (chunks[0], chunks[1])
            };

            // Draw either board or UCI debug panel based on toggle
            if app_state.ui_state.show_debug_panel {
                let uci_debug_panel = UciDebugPanel::new(app_state);
                f.render_widget(uci_debug_panel, board_area);
            } else {
                // Draw board with typeahead squares (board will scale to fill area)
                let board_widget = BoardWidget::new(app_state, &typeahead_squares);
                f.render_widget(board_widget, board_area);
            }

            // Split panels area based on layout mode
            if use_vertical_layout {
                // Vertical layout: split horizontally - left 1/3 for controls+info, right 2/3 for history
                let horizontal_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
                    .split(panels_area);

                // Left side: stack controls and game info vertically
                let left_panels = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(horizontal_split[0]);

                // Draw controls panel (top left)
                let controls_panel = ControlsPanel::new();
                f.render_widget(controls_panel, left_panels[0]);

                // Draw game info panel (bottom left)
                let game_info_panel = GameInfoPanel::new(app_state);
                f.render_widget(game_info_panel, left_panels[1]);

                // Draw move history panel (right side, takes 2/3)
                let move_history_panel = MoveHistoryPanel::new(app_state);
                f.render_widget(move_history_panel, horizontal_split[1]);
            } else {
                // Horizontal layout: stack all three panels vertically on right side
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(10), // Controls
                        Constraint::Length(12), // Game Info
                        Constraint::Min(5),     // Move History
                    ])
                    .split(panels_area);

                // Draw controls panel
                let controls_panel = ControlsPanel::new();
                f.render_widget(controls_panel, right_chunks[0]);

                // Draw game info panel
                let game_info_panel = GameInfoPanel::new(app_state);
                f.render_widget(game_info_panel, right_chunks[1]);

                // Draw move history panel
                let move_history_panel = MoveHistoryPanel::new(app_state);
                f.render_widget(move_history_panel, right_chunks[2]);
            }

            // Draw split input boxes at the bottom
            let input_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(main_chunks[1]);

            // Source square input box
            let source_active = app_state.ui_state.input_phase == InputPhase::SelectPiece;
            let source_text = if let Some(sq) = app_state.ui_state.selected_square {
                format!("Selected: {}", format_square_display(sq))
            } else if !input_buffer.is_empty() && source_active {
                format!("Typing: {}", input_buffer.as_str())
            } else {
                "Type square (e.g., e2)".to_string()
            };

            let source_style = if source_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let source_border_style = if source_active {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let source_widget =
                Paragraph::new(Line::from(vec![Span::styled(source_text, source_style)]))
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("1. Select Piece")
                            .border_style(source_border_style),
                    );
            f.render_widget(source_widget, input_chunks[0]);

            // Destination square input box
            let dest_active = app_state.ui_state.input_phase == InputPhase::SelectDestination;
            let dest_text = if !input_buffer.is_empty() && dest_active {
                format!("Typing: {}", input_buffer.as_str())
            } else if dest_active {
                "Type destination".to_string()
            } else {
                "Waiting...".to_string()
            };

            let dest_style = if dest_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let dest_border_style = if dest_active {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let dest_widget = Paragraph::new(Line::from(vec![Span::styled(dest_text, dest_style)]))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("2. Move To")
                        .border_style(dest_border_style),
                );
            f.render_widget(dest_widget, input_chunks[1]);

            // Draw promotion dialog if in SelectPromotion phase
            if let InputPhase::SelectPromotion { from, to } = app_state.ui_state.input_phase {
                let promotion_widget = PromotionWidget::new(app_state, from, to);
                f.render_widget(promotion_widget, area);
            }
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle promotion input first if in SelectPromotion phase
                if let InputPhase::SelectPromotion { from, to } = app_state.ui_state.input_phase {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            app_state.cycle_promotion_piece(-1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app_state.cycle_promotion_piece(1);
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app_state.set_promotion_piece(cozy_chess::Piece::Queen);
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app_state.set_promotion_piece(cozy_chess::Piece::Rook);
                        }
                        KeyCode::Char('b') | KeyCode::Char('B') => {
                            app_state.set_promotion_piece(cozy_chess::Piece::Bishop);
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            app_state.set_promotion_piece(cozy_chess::Piece::Knight);
                        }
                        KeyCode::Enter => {
                            let piece = app_state.ui_state.selected_promotion_piece;
                            tracing::info!("Executing promotion to {:?}", piece);
                            match app_state.execute_promotion(from, to, piece) {
                                Ok(_) => {
                                    tracing::info!("Promotion executed successfully");
                                    // Trigger engine move if it's engine's turn
                                    if app_state.is_engine_turn() {
                                        tracing::info!("Triggering engine move after promotion");
                                        if let Err(e) = app_state.make_engine_move().await {
                                            tracing::error!("Failed to trigger engine move after promotion: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Promotion failed: {}", e);
                                    app_state.ui_state.status_message = Some(format!("Error: {}", e));
                                }
                            }
                        }
                        KeyCode::Esc => {
                            app_state.cancel_promotion();
                        }
                        _ => {}
                    }
                    continue; // Skip other input handling when in promotion mode
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(false), // Quit
                    KeyCode::Char('n') => return Ok(true),  // New game (return to menu)
                    KeyCode::Char('@') => {
                        // Toggle UCI debug panel
                        app_state.toggle_debug_panel();
                    }
                    KeyCode::Char('u') => {
                        // Undo move
                        if app_state.game.undo().is_ok() {
                            app_state.ui_state.status_message = Some("Move undone".to_string());
                        }
                        app_state.clear_all_highlights();
                        input_buffer.clear();
                    }
                    KeyCode::Esc => {
                        // Clear selection and input
                        app_state.clear_all_highlights();
                        input_buffer.clear();
                        app_state.ui_state.status_message = Some("Selection cleared".to_string());
                    }
                    KeyCode::Enter => {
                        // Clear any stale highlights
                        if app_state.ui_state.selected_square.is_none() {
                            app_state.clear_all_highlights();
                        }
                        input_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        input_buffer.backspace();
                    }
                    KeyCode::Char(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {
                        input_buffer.push_char(c);

                        // When input is complete, try to parse as square
                        if input_buffer.is_complete() {
                            if let Some(square) = input_buffer.try_parse_square() {
                                // Check if we already have a square selected
                                if app_state.ui_state.selected_square.is_some() {
                                    // This is the destination square
                                    match app_state.try_move_to(square) {
                                        Ok(_) => {
                                            tracing::info!("Human move completed successfully");
                                            // Move successful - highlights already cleared by try_move_to
                                            input_buffer.clear();

                                            // Trigger engine move if it's engine's turn
                                            if app_state.is_engine_turn() {
                                                tracing::info!("Triggering engine move after human move");
                                                if let Err(e) = app_state.make_engine_move().await {
                                                    tracing::error!("Failed to trigger engine move after human move: {}", e);
                                                }
                                            }
                                        }
                                        Err(_e) => {
                                            // Move failed, clear old selection and try selecting this square instead
                                            app_state.clear_all_highlights();
                                            app_state.select_square(square);
                                            input_buffer.clear();
                                        }
                                    }
                                } else {
                                    // This is the source square - clear any previous highlights first
                                    app_state.clear_all_highlights();
                                    app_state.select_square(square);
                                    input_buffer.clear();
                                }
                            } else {
                                // Invalid square notation
                                app_state.ui_state.status_message =
                                    Some("Invalid square!".to_string());
                                input_buffer.clear();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
