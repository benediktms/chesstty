use crate::state::{ClientState, GameMode, InputPhase, PlayerColor, UiState};
use crate::ui::widgets::{
    BoardWidget, ControlsPanel, FenDialogFocus, FenDialogState, FenDialogWidget, GameInfoPanel,
    MenuState, MenuWidget, MoveHistoryPanel, PromotionWidget, UciDebugPanel,
};
use crate::ui::widgets::menu::StartPositionOption;
use crate::converters::{parse_square, format_square};
use cozy_chess::{Board, Square};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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
use std::time::{Duration, Instant};

enum AppMode {
    Menu,
    Game,
}

/// Temporary state for menu navigation
struct MenuContext {
    menu_state: MenuState,
    fen_history: Vec<String>, // Simple FEN history for now
}

impl MenuContext {
    fn new() -> Self {
        Self {
            menu_state: MenuState {
                selected_index: 0,
                game_mode: crate::ui::widgets::menu::GameModeOption::HumanVsEngine,
                difficulty: crate::ui::widgets::menu::DifficultyOption::Intermediate,
                time_control: crate::ui::widgets::menu::TimeControlOption::None,
                start_position: StartPositionOption::Standard,
                fen_dialog_state: None,
                fen_history: vec![
                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(), // Standard
                ],
            },
            fen_history: vec![
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            ],
        }
    }
}

/// Run the TUI application
pub async fn run_app() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut menu_context = MenuContext::new();
    let mut app_mode = AppMode::Menu;
    let mut client_state: Option<ClientState> = None;
    let mut input_buffer = String::new();
    let mut typeahead_squares: Vec<Square> = Vec::new();

    let result = loop {
        // Render based on current mode
        terminal.draw(|f| {
            match &app_mode {
                AppMode::Menu => {
                    let menu_widget = MenuWidget {
                        state: &menu_context.menu_state,
                    };
                    f.render_widget(menu_widget, f.size());
                }
                AppMode::Game => {
                    if let Some(state) = &client_state {
                        render_game(f, state, &input_buffer, &typeahead_squares);
                    }
                }
            }
        })?;

        // Handle input with timeout for non-blocking updates
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match &mut app_mode {
                    AppMode::Menu => {
                        let result = handle_menu_input(&mut menu_context, key.code).await;
                        match result {
                            MenuAction::StartGame(start_fen) => {
                                // Connect to server and create game
                                match ClientState::new("http://[::1]:50051").await {
                                    Ok(mut state) => {
                                        // Reset to starting position if needed
                                        if let Some(fen) = start_fen {
                                            if let Err(e) = state.reset(Some(fen)).await {
                                                state.ui_state.status_message = Some(format!("Error loading FEN: {}", e));
                                            }
                                        }

                                        // Set game mode
                                        state.mode = match menu_context.menu_state.game_mode {
                                            crate::ui::widgets::menu::GameModeOption::HumanVsHuman => GameMode::HumanVsHuman,
                                            crate::ui::widgets::menu::GameModeOption::HumanVsEngine => {
                                                let difficulty = menu_context.menu_state.difficulty;
                                                let skill_level = match difficulty {
                                                    crate::ui::widgets::menu::DifficultyOption::Beginner => 1,
                                                    crate::ui::widgets::menu::DifficultyOption::Easy => 5,
                                                    crate::ui::widgets::menu::DifficultyOption::Intermediate => 10,
                                                    crate::ui::widgets::menu::DifficultyOption::Advanced => 15,
                                                    crate::ui::widgets::menu::DifficultyOption::Master => 20,
                                                };
                                                state.skill_level = skill_level;
                                                let _ = state.set_engine(true, skill_level).await;
                                                GameMode::HumanVsEngine { human_side: PlayerColor::White }
                                            }
                                            crate::ui::widgets::menu::GameModeOption::EngineVsEngine => GameMode::EngineVsEngine,
                                        };

                                        client_state = Some(state);
                                        app_mode = AppMode::Game;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to connect to server: {}", e);
                                        // TODO: Show error in menu
                                    }
                                }
                            }
                            MenuAction::Continue => {}
                            MenuAction::Quit => break Ok(()),
                        }
                    }
                    AppMode::Game => {
                        if let Some(state) = &mut client_state {
                            let should_quit = handle_game_input(
                                state,
                                &mut input_buffer,
                                &mut typeahead_squares,
                                key.code,
                                key.modifiers,
                            ).await;

                            if should_quit {
                                // Return to menu
                                client_state = None;
                                input_buffer.clear();
                                typeahead_squares.clear();
                                app_mode = AppMode::Menu;
                            }
                        }
                    }
                }
            }
        }

        // Update engine moves if needed
        if let Some(state) = &mut client_state {
            if state.is_engine_turn() {
                if let Err(e) = state.make_engine_move().await {
                    state.ui_state.status_message = Some(format!("Engine error: {}", e));
                }
            }
        }
    };

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

enum MenuAction {
    StartGame(Option<String>), // Optional FEN
    Continue,
    Quit,
}

async fn handle_menu_input(context: &mut MenuContext, key: KeyCode) -> MenuAction {
    use crate::ui::widgets::menu::{GameModeOption, DifficultyOption, TimeControlOption};

    // Handle FEN dialog if active
    if let Some(ref mut dialog_state) = context.menu_state.fen_dialog_state {
        match key {
            KeyCode::Esc => {
                context.menu_state.fen_dialog_state = None;
                return MenuAction::Continue;
            }
            KeyCode::Enter => {
                // Use the entered FEN
                let fen = dialog_state.input.clone();
                context.menu_state.fen_dialog_state = None;
                if !fen.is_empty() {
                    return MenuAction::StartGame(Some(fen));
                }
                return MenuAction::Continue;
            }
            KeyCode::Char(c) => {
                dialog_state.input.push(c);
                return MenuAction::Continue;
            }
            KeyCode::Backspace => {
                dialog_state.input.pop();
                return MenuAction::Continue;
            }
            KeyCode::Up => {
                // Navigate history
                if dialog_state.history_selected_index > 0 {
                    dialog_state.history_selected_index -= 1;
                    if let Some(fen) = context.fen_history.get(dialog_state.history_selected_index) {
                        dialog_state.input = fen.clone();
                    }
                }
                return MenuAction::Continue;
            }
            KeyCode::Down => {
                // Navigate history
                if dialog_state.history_selected_index < context.fen_history.len().saturating_sub(1) {
                    dialog_state.history_selected_index += 1;
                    if let Some(fen) = context.fen_history.get(dialog_state.history_selected_index) {
                        dialog_state.input = fen.clone();
                    }
                }
                return MenuAction::Continue;
            }
            KeyCode::Tab => {
                // Switch focus
                dialog_state.focus = match dialog_state.focus {
                    FenDialogFocus::Input => FenDialogFocus::History,
                    FenDialogFocus::History => FenDialogFocus::Input,
                };
                return MenuAction::Continue;
            }
            _ => return MenuAction::Continue,
        }
    }

    // Handle normal menu navigation
    match key {
        KeyCode::Up => {
            if context.menu_state.selected_index > 0 {
                context.menu_state.selected_index -= 1;
            }
        }
        KeyCode::Down => {
            if context.menu_state.selected_index < 3 {
                context.menu_state.selected_index += 1;
            }
        }
        KeyCode::Left => {
            match context.menu_state.selected_index {
                0 => {
                    // Cycle game mode
                    context.menu_state.game_mode = match context.menu_state.game_mode {
                        GameModeOption::HumanVsHuman => GameModeOption::EngineVsEngine,
                        GameModeOption::HumanVsEngine => GameModeOption::HumanVsHuman,
                        GameModeOption::EngineVsEngine => GameModeOption::HumanVsEngine,
                    };
                }
                1 => {
                    // Cycle difficulty
                    context.menu_state.difficulty = match context.menu_state.difficulty {
                        DifficultyOption::Beginner => DifficultyOption::Master,
                        DifficultyOption::Easy => DifficultyOption::Beginner,
                        DifficultyOption::Intermediate => DifficultyOption::Easy,
                        DifficultyOption::Advanced => DifficultyOption::Intermediate,
                        DifficultyOption::Master => DifficultyOption::Advanced,
                    };
                }
                2 => {
                    // Cycle time control
                    context.menu_state.time_control = match context.menu_state.time_control {
                        TimeControlOption::None => TimeControlOption::Classical,
                        TimeControlOption::Bullet => TimeControlOption::None,
                        TimeControlOption::Blitz => TimeControlOption::Bullet,
                        TimeControlOption::Rapid => TimeControlOption::Blitz,
                        TimeControlOption::Classical => TimeControlOption::Rapid,
                    };
                }
                3 => {
                    // Cycle start position
                    context.menu_state.start_position = match context.menu_state.start_position {
                        StartPositionOption::Standard => StartPositionOption::Custom,
                        StartPositionOption::Custom => StartPositionOption::Standard,
                    };
                }
                _ => {}
            }
        }
        KeyCode::Right => {
            match context.menu_state.selected_index {
                0 => {
                    context.menu_state.game_mode = match context.menu_state.game_mode {
                        GameModeOption::HumanVsHuman => GameModeOption::HumanVsEngine,
                        GameModeOption::HumanVsEngine => GameModeOption::EngineVsEngine,
                        GameModeOption::EngineVsEngine => GameModeOption::HumanVsHuman,
                    };
                }
                1 => {
                    context.menu_state.difficulty = match context.menu_state.difficulty {
                        DifficultyOption::Beginner => DifficultyOption::Easy,
                        DifficultyOption::Easy => DifficultyOption::Intermediate,
                        DifficultyOption::Intermediate => DifficultyOption::Advanced,
                        DifficultyOption::Advanced => DifficultyOption::Master,
                        DifficultyOption::Master => DifficultyOption::Beginner,
                    };
                }
                2 => {
                    context.menu_state.time_control = match context.menu_state.time_control {
                        TimeControlOption::None => TimeControlOption::Bullet,
                        TimeControlOption::Bullet => TimeControlOption::Blitz,
                        TimeControlOption::Blitz => TimeControlOption::Rapid,
                        TimeControlOption::Rapid => TimeControlOption::Classical,
                        TimeControlOption::Classical => TimeControlOption::None,
                    };
                }
                3 => {
                    context.menu_state.start_position = match context.menu_state.start_position {
                        StartPositionOption::Standard => StartPositionOption::Custom,
                        StartPositionOption::Custom => StartPositionOption::Standard,
                    };
                }
                _ => {}
            }
        }
        KeyCode::Enter => {
            // Check if custom FEN selected
            if context.menu_state.selected_index == 3
                && matches!(context.menu_state.start_position, StartPositionOption::Custom) {
                // Show FEN dialog
                context.menu_state.fen_dialog_state = Some(FenDialogState {
                    input: String::new(),
                    history_selected_index: 0,
                    focus: FenDialogFocus::Input,
                });
                return MenuAction::Continue;
            }

            // Start game with standard position
            return MenuAction::StartGame(None);
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            return MenuAction::Quit;
        }
        _ => {}
    }

    MenuAction::Continue
}

async fn handle_game_input(
    state: &mut ClientState,
    input_buffer: &mut String,
    typeahead_squares: &mut Vec<Square>,
    key: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    // Handle promotion dialog first
    if let InputPhase::SelectPromotion { from, to } = state.ui_state.input_phase {
        match key {
            KeyCode::Char('q') => {
                let _ = state.execute_promotion(from, to, cozy_chess::Piece::Queen).await;
                return false;
            }
            KeyCode::Char('r') => {
                let _ = state.execute_promotion(from, to, cozy_chess::Piece::Rook).await;
                return false;
            }
            KeyCode::Char('b') => {
                let _ = state.execute_promotion(from, to, cozy_chess::Piece::Bishop).await;
                return false;
            }
            KeyCode::Char('n') => {
                let _ = state.execute_promotion(from, to, cozy_chess::Piece::Knight).await;
                return false;
            }
            KeyCode::Esc => {
                state.cancel_promotion();
                return false;
            }
            _ => return false,
        }
    }

    match key {
        KeyCode::Char('@') => {
            state.toggle_debug_panel();
        }
        KeyCode::Esc => {
            // Return to menu
            return true;
        }
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            return true;
        }
        KeyCode::Char(c) => {
            input_buffer.push(c);

            // Update typeahead squares
            *typeahead_squares = state.filter_selectable_by_input(input_buffer);

            // Check if we have a complete square (2 characters)
            if input_buffer.len() == 2 {
                if let Some(square) = parse_square(input_buffer) {
                    match state.ui_state.input_phase {
                        InputPhase::SelectPiece => {
                            if state.ui_state.selectable_squares.contains(&square) {
                                state.select_square(square);
                            } else {
                                state.ui_state.status_message = Some("No piece on that square".to_string());
                            }
                        }
                        InputPhase::SelectDestination => {
                            let _ = state.try_move_to(square).await;
                        }
                        _ => {}
                    }
                }
                input_buffer.clear();
                typeahead_squares.clear();
            }
        }
        KeyCode::Backspace => {
            input_buffer.pop();
            *typeahead_squares = state.filter_selectable_by_input(input_buffer);
        }
        KeyCode::Enter => {
            input_buffer.clear();
            typeahead_squares.clear();
        }
        _ => {}
    }

    false
}

fn render_game(
    f: &mut ratatui::Frame,
    state: &ClientState,
    input_buffer: &str,
    typeahead_squares: &[Square],
) {
    let size = f.size();

    // Main layout: board on left, panels on right
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(128), // Board needs 128 chars
            Constraint::Percentage(100),
        ])
        .split(size);

    // Right side: info panels stacked
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // Game info
            Constraint::Length(15), // Move history
            Constraint::Length(8),  // Controls
            Constraint::Min(0),     // UCI debug (if shown)
        ])
        .split(main_chunks[1]);

    // Render board - need to adapt to use ClientState
    let board_widget = create_adapted_board_widget(state, typeahead_squares);
    f.render_widget(board_widget, main_chunks[0]);

    // Render game info panel
    let game_info = create_game_info_panel(state);
    f.render_widget(game_info, right_chunks[0]);

    // Render move history
    let history_widget = create_move_history_panel(state);
    f.render_widget(history_widget, right_chunks[1]);

    // Render controls panel
    let controls = create_controls_panel(state, input_buffer);
    f.render_widget(controls, right_chunks[2]);

    // Render UCI debug panel if shown
    if state.ui_state.show_debug_panel {
        let uci_panel = create_uci_debug_panel(state);
        f.render_widget(uci_panel, right_chunks[3]);
    }

    // Render promotion dialog if active
    if let InputPhase::SelectPromotion { .. } = state.ui_state.input_phase {
        let promotion_widget = PromotionWidget {
            selected_piece: state.ui_state.selected_promotion_piece,
        };

        // Center the promotion dialog
        let dialog_area = centered_rect(60, 40, size);
        f.render_widget(promotion_widget, dialog_area);
    }
}

// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Adapter to create BoardWidget from ClientState
// Since BoardWidget expects a reference with game.position(), we need a wrapper
struct ClientStateAdapter<'a> {
    state: &'a ClientState,
}

impl<'a> ClientStateAdapter<'a> {
    fn position(&self) -> &Board {
        self.state.board()
    }

    fn ui_state(&self) -> &UiState {
        &self.state.ui_state
    }
}

fn create_adapted_board_widget<'a>(
    state: &'a ClientState,
    typeahead_squares: &'a [Square],
) -> impl ratatui::widgets::Widget + 'a {
    // For now, create a simple adapter
    // TODO: Update BoardWidget to work directly with ClientState
    BoardWidget {
        app_state: unsafe {
            // This is a temporary hack - we need to refactor BoardWidget
            // For now, transmute the ClientState reference
            std::mem::transmute(state)
        },
        typeahead_squares,
    }
}

fn create_game_info_panel(state: &ClientState) -> GameInfoPanel<'_> {
    GameInfoPanel {
        app_state: unsafe { std::mem::transmute(state) },
    }
}

fn create_move_history_panel(state: &ClientState) -> MoveHistoryPanel<'_> {
    MoveHistoryPanel {
        history: state.history(),
    }
}

fn create_controls_panel<'a>(
    state: &'a ClientState,
    input_buffer: &'a str,
) -> ControlsPanel<'a> {
    ControlsPanel {
        input_buffer,
        status_message: state.ui_state.status_message.as_deref(),
        input_phase: state.ui_state.input_phase,
    }
}

fn create_uci_debug_panel(state: &ClientState) -> UciDebugPanel<'_> {
    UciDebugPanel {
        uci_log: &state.ui_state.uci_log,
    }
}
