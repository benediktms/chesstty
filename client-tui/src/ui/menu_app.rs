use crate::state::{GameMode, PlayerColor};
use crate::ui::widgets::selectable_table::SelectableTableState;
use crate::ui::widgets::{FenDialogState, FenDialogWidget, MenuState, MenuWidget, render_table_overlay};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Constraint, Terminal};
use std::io;
use std::time::Duration;

pub struct GameConfig {
    pub mode: GameMode,
    pub skill_level: u8,
    pub start_fen: Option<String>,
    pub time_control_seconds: Option<u64>,
    pub engine_threads: Option<u32>,
    pub engine_hash_mb: Option<u32>,
    /// If set, resume this suspended session by ID instead of starting a new game.
    pub resume_session_id: Option<String>,
    /// Metadata from the suspended session (game mode, skill level etc.)
    pub resume_game_mode: Option<String>,
    pub resume_human_side: Option<String>,
    pub resume_skill_level: Option<u8>,
}

/// Show menu and get game configuration.
/// Pre-fetched data from the server is passed in to avoid async calls during menu rendering.
pub async fn show_menu(
    suspended_sessions: Vec<chess_proto::SuspendedSessionInfo>,
    saved_positions: Vec<chess_proto::SavedPosition>,
) -> anyhow::Result<Option<GameConfig>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut menu_state = MenuState::default();
    menu_state.has_saved_session = !suspended_sessions.is_empty();
    menu_state.suspended_sessions = suspended_sessions;
    menu_state.saved_positions = saved_positions;
    let result = loop {
        terminal.draw(|f| {
            let menu_widget = MenuWidget {
                menu_state: &menu_state,
            };
            f.render_widget(menu_widget, f.area());

            // Render FEN dialog if active
            if let Some(ref mut dialog_state) = menu_state.fen_dialog_state {
                let fen_dialog = FenDialogWidget::new(dialog_state, &menu_state.saved_positions);
                f.render_widget(fen_dialog, f.area());
            }

            // Render session selection table if active
            if let Some(ref mut ctx) = menu_state.session_table {
                let rows: Vec<Vec<String>> = ctx.sessions.iter().map(|s| {
                    let mode = s.game_mode.as_str();
                    let moves = format!("{} moves", s.move_count);
                    let side = s.side_to_move.clone();
                    let fen_preview = if s.fen.len() > 30 {
                        format!("{}...", &s.fen[..27])
                    } else {
                        s.fen.clone()
                    };
                    vec![mode.to_string(), moves, side, fen_preview]
                }).collect();

                render_table_overlay(
                    f.area(),
                    f.buffer_mut(),
                    "Resume Session",
                    &["Mode", "Moves", "Turn", "Position"],
                    &rows,
                    &[Constraint::Length(16), Constraint::Length(10), Constraint::Length(8), Constraint::Min(20)],
                    &mut ctx.table_state,
                    70,
                    (ctx.sessions.len() as u16 + 5).min(20),
                );
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Session table takes highest priority
                if menu_state.session_table.is_some() {
                    let action = handle_session_table_input(&mut menu_state, key.code);
                    if let Some(config) = action {
                        break Some(config);
                    }
                    continue;
                }

                // Handle FEN dialog input if active
                if menu_state.fen_dialog_state.is_some() {
                    handle_fen_dialog_input(&mut menu_state, key.code);
                    continue;
                }

                // Handle menu navigation
                use crate::ui::widgets::menu::MenuItem;

                let items = menu_state.items();
                let selected_item = items.get(menu_state.selected_index).cloned();

                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if menu_state.selected_index > 0 {
                            menu_state.selected_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max_index = items.len().saturating_sub(1);
                        if menu_state.selected_index < max_index {
                            menu_state.selected_index += 1;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        cycle_option(&mut menu_state, &selected_item, -1);
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        cycle_option(&mut menu_state, &selected_item, 1);
                    }
                    KeyCode::Enter => {
                        match selected_item {
                            Some(MenuItem::StartGame) => {
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen
                                    && menu_state.selected_fen.is_none()
                                {
                                    menu_state.fen_dialog_state = Some(FenDialogState::new(menu_state.saved_positions.len()));
                                } else {
                                    let config = create_game_config(&menu_state);
                                    break Some(config);
                                }
                            }
                            Some(MenuItem::ResumeSession) => {
                                let sessions = menu_state.suspended_sessions.clone();
                                if !sessions.is_empty() {
                                    use crate::ui::widgets::menu::SessionTableContext;
                                    let count = sessions.len();
                                    menu_state.session_table = Some(SessionTableContext {
                                        table_state: SelectableTableState::new(count),
                                        sessions,
                                    });
                                }
                            }
                            Some(MenuItem::Quit) => {
                                break None;
                            }
                            Some(MenuItem::StartPosition(_)) => {
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen {
                                    menu_state.fen_dialog_state = Some(FenDialogState::new(menu_state.saved_positions.len()));
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break None;
                    }
                    _ => {}
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}

fn cycle_option(menu_state: &mut MenuState, selected_item: &Option<crate::ui::widgets::menu::MenuItem>, _direction: i32) {
    use crate::ui::widgets::menu::{MenuItem, DifficultyOption, GameModeOption, TimeControlOption, StartPositionOption};

    let Some(item) = selected_item else { return };

    match item {
        MenuItem::GameMode(_) => {
            menu_state.game_mode = match menu_state.game_mode {
                GameModeOption::HumanVsHuman => {
                    if _direction > 0 { GameModeOption::HumanVsEngine } else { GameModeOption::EngineVsEngine }
                }
                GameModeOption::HumanVsEngine => {
                    if _direction > 0 { GameModeOption::EngineVsEngine } else { GameModeOption::HumanVsHuman }
                }
                GameModeOption::EngineVsEngine => {
                    if _direction > 0 { GameModeOption::HumanVsHuman } else { GameModeOption::HumanVsEngine }
                }
            };
        }
        MenuItem::PlayAs(_) => {
            menu_state.cycle_play_as();
        }
        MenuItem::Difficulty(_) => {
            menu_state.difficulty = match menu_state.difficulty {
                DifficultyOption::Beginner => {
                    if _direction > 0 { DifficultyOption::Intermediate } else { DifficultyOption::Master }
                }
                DifficultyOption::Intermediate => {
                    if _direction > 0 { DifficultyOption::Advanced } else { DifficultyOption::Beginner }
                }
                DifficultyOption::Advanced => {
                    if _direction > 0 { DifficultyOption::Master } else { DifficultyOption::Intermediate }
                }
                DifficultyOption::Master => {
                    if _direction > 0 { DifficultyOption::Beginner } else { DifficultyOption::Advanced }
                }
            };
        }
        MenuItem::TimeControl(_) => {
            menu_state.time_control = match menu_state.time_control {
                TimeControlOption::None => {
                    if _direction > 0 { TimeControlOption::Blitz } else { TimeControlOption::Classical }
                }
                TimeControlOption::Blitz => {
                    if _direction > 0 { TimeControlOption::Rapid } else { TimeControlOption::None }
                }
                TimeControlOption::Rapid => {
                    if _direction > 0 { TimeControlOption::Classical } else { TimeControlOption::Blitz }
                }
                TimeControlOption::Classical => {
                    if _direction > 0 { TimeControlOption::None } else { TimeControlOption::Rapid }
                }
            };
        }
        MenuItem::EngineThreads(_) => {
            use crate::ui::widgets::menu::ThreadsOption;
            menu_state.engine_threads = match menu_state.engine_threads {
                ThreadsOption::Auto => {
                    if _direction > 0 { ThreadsOption::One } else { ThreadsOption::Four }
                }
                ThreadsOption::One => {
                    if _direction > 0 { ThreadsOption::Two } else { ThreadsOption::Auto }
                }
                ThreadsOption::Two => {
                    if _direction > 0 { ThreadsOption::Four } else { ThreadsOption::One }
                }
                ThreadsOption::Four => {
                    if _direction > 0 { ThreadsOption::Auto } else { ThreadsOption::Two }
                }
            };
        }
        MenuItem::EngineHash(_) => {
            use crate::ui::widgets::menu::HashOption;
            menu_state.engine_hash = match menu_state.engine_hash {
                HashOption::Small => {
                    if _direction > 0 { HashOption::Medium } else { HashOption::Large }
                }
                HashOption::Medium => {
                    if _direction > 0 { HashOption::Large } else { HashOption::Small }
                }
                HashOption::Large => {
                    if _direction > 0 { HashOption::Small } else { HashOption::Medium }
                }
            };
        }
        MenuItem::StartPosition(_) => {
            menu_state.start_position = match menu_state.start_position {
                StartPositionOption::Standard => StartPositionOption::CustomFen,
                StartPositionOption::CustomFen => StartPositionOption::Standard,
            };
        }
        _ => {}
    }
}

fn handle_fen_dialog_input(menu_state: &mut MenuState, key_code: KeyCode) {
    use crate::ui::widgets::fen_dialog::FenDialogFocus;

    let dialog_state = match &mut menu_state.fen_dialog_state {
        Some(state) => state,
        None => return,
    };

    match key_code {
        KeyCode::Esc => {
            menu_state.fen_dialog_state = None;
        }
        KeyCode::Enter => {
            match dialog_state.focus {
                FenDialogFocus::Input => {
                    // Use the FEN from the input buffer
                    let fen = dialog_state.input_buffer.trim().to_string();
                    if !fen.is_empty() {
                        if validate_fen_basic(&fen) {
                            // Server will do full validation when creating the session.
                            // For now, just accept it.
                            menu_state.selected_fen = Some(fen);
                            menu_state.fen_dialog_state = None;
                        } else {
                            dialog_state.validation_error = Some("Invalid FEN format".to_string());
                        }
                    } else {
                        dialog_state.validation_error = Some("FEN cannot be empty".to_string());
                    }
                }
                FenDialogFocus::PositionList => {
                    // Select the FEN from the positions table
                    if let Some(idx) = dialog_state.position_table.selected_index() {
                        if let Some(pos) = menu_state.saved_positions.get(idx) {
                            menu_state.selected_fen = Some(pos.fen.clone());
                            menu_state.fen_dialog_state = None;
                        }
                    }
                }
            }
        }
        KeyCode::Tab => {
            dialog_state.focus = match dialog_state.focus {
                FenDialogFocus::Input => FenDialogFocus::PositionList,
                FenDialogFocus::PositionList => FenDialogFocus::Input,
            };
        }
        KeyCode::Char(c) => {
            if dialog_state.focus == FenDialogFocus::Input {
                dialog_state.input_buffer.push(c);
                dialog_state.validation_error = None;
            } else if dialog_state.focus == FenDialogFocus::PositionList {
                match c {
                    'k' => dialog_state.position_table.move_up(),
                    'j' => dialog_state.position_table.move_down(),
                    'd' => {
                        // Delete selected position (not defaults)
                        if let Some(idx) = dialog_state.position_table.selected_index() {
                            if let Some(pos) = menu_state.saved_positions.get(idx) {
                                if !pos.is_default {
                                    menu_state.saved_positions.remove(idx);
                                    dialog_state.position_table.update_row_count(
                                        menu_state.saved_positions.len(),
                                    );
                                    // Note: actual server deletion happens via RPC
                                    // (would need async; for now just remove from local list)
                                } else {
                                    dialog_state.validation_error =
                                        Some("Cannot delete default positions".to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Backspace => {
            if dialog_state.focus == FenDialogFocus::Input {
                dialog_state.input_buffer.pop();
                dialog_state.validation_error = None;
            }
        }
        KeyCode::Up => {
            if dialog_state.focus == FenDialogFocus::PositionList {
                dialog_state.position_table.move_up();
            }
        }
        KeyCode::Down => {
            if dialog_state.focus == FenDialogFocus::PositionList {
                dialog_state.position_table.move_down();
            }
        }
        _ => {}
    }
}

fn validate_fen_basic(fen: &str) -> bool {
    // Basic FEN validation: should have 6 space-separated parts
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() < 4 {
        return false;
    }

    // First part should be the board (8 ranks separated by /)
    let board_part = parts[0];
    let ranks: Vec<&str> = board_part.split('/').collect();
    if ranks.len() != 8 {
        return false;
    }

    // Second part should be side to move (w or b)
    if parts[1] != "w" && parts[1] != "b" {
        return false;
    }

    true
}

fn create_game_config(menu_state: &MenuState) -> GameConfig {
    use crate::ui::widgets::menu::{DifficultyOption, GameModeOption, PlayAsOption};

    let mode = match menu_state.game_mode {
        GameModeOption::HumanVsHuman => GameMode::HumanVsHuman,
        GameModeOption::HumanVsEngine => {
            let human_side = match menu_state.play_as {
                PlayAsOption::White => PlayerColor::White,
                PlayAsOption::Black => PlayerColor::Black,
            };
            GameMode::HumanVsEngine { human_side }
        }
        GameModeOption::EngineVsEngine => GameMode::EngineVsEngine,
    };

    let skill_level = match menu_state.difficulty {
        DifficultyOption::Beginner => 3,
        DifficultyOption::Intermediate => 10,
        DifficultyOption::Advanced => 15,
        DifficultyOption::Master => 20,
    };

    let start_fen = menu_state.selected_fen.clone();
    let time_control_seconds = menu_state.time_control.seconds();

    let has_engine = matches!(
        menu_state.game_mode,
        GameModeOption::HumanVsEngine | GameModeOption::EngineVsEngine
    );
    let engine_threads = if has_engine {
        Some(menu_state.engine_threads.resolve())
    } else {
        None
    };
    let engine_hash_mb = if has_engine {
        Some(menu_state.engine_hash.megabytes())
    } else {
        None
    };

    GameConfig {
        mode,
        skill_level,
        start_fen,
        time_control_seconds,
        engine_threads,
        engine_hash_mb,
        resume_session_id: None,
        resume_game_mode: None,
        resume_human_side: None,
        resume_skill_level: None,
    }
}

/// Handle input for the session selection table.
/// Returns Some(GameConfig) when a session is selected, None to continue.
fn handle_session_table_input(menu_state: &mut MenuState, key_code: KeyCode) -> Option<GameConfig> {
    let ctx = menu_state.session_table.as_mut()?;

    match key_code {
        KeyCode::Up | KeyCode::Char('k') => {
            ctx.table_state.move_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            ctx.table_state.move_down();
        }
        KeyCode::Enter => {
            if let Some(idx) = ctx.table_state.selected_index() {
                if let Some(session) = ctx.sessions.get(idx).cloned() {
                    menu_state.session_table = None;
                    let mut config = create_game_config(menu_state);
                    config.resume_session_id = Some(session.suspended_id);
                    config.resume_game_mode = Some(session.game_mode);
                    config.resume_human_side = session.human_side;
                    config.resume_skill_level = Some(session.skill_level as u8);
                    return Some(config);
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            // Delete the selected session (will be handled by caller via RPC later;
            // for now just remove from the local list)
            if let Some(idx) = ctx.table_state.selected_index() {
                if idx < ctx.sessions.len() {
                    ctx.sessions.remove(idx);
                    ctx.table_state.update_row_count(ctx.sessions.len());
                    // Update the main menu's session list too
                    menu_state.suspended_sessions = ctx.sessions.clone();
                    menu_state.has_saved_session = !ctx.sessions.is_empty();
                    if ctx.sessions.is_empty() {
                        menu_state.session_table = None;
                    }
                }
            }
        }
        KeyCode::Esc => {
            menu_state.session_table = None;
        }
        _ => {}
    }

    None
}
