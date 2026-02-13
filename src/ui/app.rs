use crate::app::{AppState, GameMode, InputBuffer};
use crate::ui::widgets::{BoardWidget, ControlsPanel, MenuWidget, MenuState};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Span,
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

    // Create menu state
    let mut menu_state = MenuState::default();
    let mut app_mode = AppMode::Menu;
    let mut app_state = AppState::new();

    // Run main loop
    let result = loop {
        match app_mode {
            AppMode::Menu => {
                terminal.draw(|f| {
                    let menu_widget = MenuWidget::new(&menu_state);
                    f.render_widget(menu_widget, f.area());
                })?;

                // Handle menu input
                if event::poll(std::time::Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Char('q') => break Ok(()),
                            KeyCode::Up => menu_state.move_up(),
                            KeyCode::Down => menu_state.move_down(menu_state.items().len()),
                            KeyCode::Left => {
                                match menu_state.selected_index {
                                    0 => menu_state.cycle_game_mode(),
                                    1 => menu_state.cycle_difficulty(),
                                    2 => menu_state.cycle_time_control(),
                                    _ => {}
                                }
                            }
                            KeyCode::Right => {
                                match menu_state.selected_index {
                                    0 => menu_state.cycle_game_mode(),
                                    1 => menu_state.cycle_difficulty(),
                                    2 => menu_state.cycle_time_control(),
                                    _ => {}
                                }
                            }
                            KeyCode::Enter => {
                                match menu_state.selected_index {
                                    3 => {
                                        // Start Game
                                        app_state = create_game_from_menu(&menu_state);
                                        app_mode = AppMode::Game;
                                    }
                                    4 => {
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

    let mut app_state = AppState::new();
    app_state.mode = mode;

    // TODO: Store difficulty and time control in app state
    app_state
}

async fn run_game_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app_state: &mut AppState,
) -> anyhow::Result<bool> {
    let mut input_buffer = InputBuffer::new();

    loop {
        // Draw UI
        terminal.draw(|f| {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(3)])
                .split(f.area());

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(50), Constraint::Length(35)])
                .split(main_chunks[0]);

            // Draw board
            let board_widget = BoardWidget::new(app_state);
            f.render_widget(board_widget, chunks[0]);

            // Draw controls panel
            let controls_panel = ControlsPanel::new(app_state);
            f.render_widget(controls_panel, chunks[1]);

            // Draw input buffer at the bottom
            let input_text = if input_buffer.is_empty() {
                "Type a square (e.g., e2): ".to_string()
            } else {
                format!("Input: {} ", input_buffer.as_str())
            };

            let input_widget = Paragraph::new(Span::styled(
                input_text,
                Style::default().fg(Color::Yellow),
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Move Input")
                    .border_style(Style::default().fg(Color::Cyan)),
            );
            f.render_widget(input_widget, main_chunks[1]);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(false), // Quit
                    KeyCode::Char('n') => return Ok(true),  // New game (return to menu)
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
                                            // Move successful - highlights already cleared by try_move_to
                                            input_buffer.clear();
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
