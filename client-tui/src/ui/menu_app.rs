use crate::state::{GameMode, PlayerColor};
use crate::ui::widgets::{FenDialogState, FenDialogWidget, MenuState, MenuWidget};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

pub struct GameConfig {
    pub mode: GameMode,
    pub skill_level: u8,
    pub start_fen: Option<String>,
    pub time_control_seconds: Option<u64>,
    pub resume: bool,
}

/// Show menu and get game configuration
pub async fn show_menu() -> anyhow::Result<Option<GameConfig>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut menu_state = MenuState::default();
    // Check for saved session
    menu_state.has_saved_session = crate::session_file::load_session()
        .ok()
        .flatten()
        .is_some();
    let result = loop {
        terminal.draw(|f| {
            let menu_widget = MenuWidget {
                menu_state: &menu_state,
            };
            f.render_widget(menu_widget, f.area());

            // Render FEN dialog if active
            if let Some(ref dialog_state) = menu_state.fen_dialog_state {
                let fen_dialog = FenDialogWidget::new(dialog_state, &menu_state.fen_history);
                f.render_widget(fen_dialog, f.area());
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
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
                    KeyCode::Up => {
                        if menu_state.selected_index > 0 {
                            menu_state.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let max_index = items.len().saturating_sub(1);
                        if menu_state.selected_index < max_index {
                            menu_state.selected_index += 1;
                        }
                    }
                    KeyCode::Left => {
                        cycle_option(&mut menu_state, &selected_item, -1);
                    }
                    KeyCode::Right => {
                        cycle_option(&mut menu_state, &selected_item, 1);
                    }
                    KeyCode::Enter => {
                        match selected_item {
                            Some(MenuItem::StartGame) => {
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen
                                    && menu_state.selected_fen.is_none()
                                {
                                    menu_state.fen_dialog_state = Some(FenDialogState::new());
                                } else {
                                    let config = create_game_config(&menu_state);
                                    break Some(config);
                                }
                            }
                            Some(MenuItem::ResumeSession) => {
                                let mut config = create_game_config(&menu_state);
                                config.resume = true;
                                break Some(config);
                            }
                            Some(MenuItem::Quit) => {
                                break None;
                            }
                            Some(MenuItem::StartPosition(_)) => {
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen {
                                    menu_state.fen_dialog_state = Some(FenDialogState::new());
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
            // Cancel and close dialog
            menu_state.fen_dialog_state = None;
        }
        KeyCode::Enter => {
            // Confirm FEN
            let fen = dialog_state.input_buffer.trim().to_string();
            if !fen.is_empty() {
                // Basic FEN validation (check if it has the right structure)
                if validate_fen_basic(&fen) {
                    // Add to history if not already present
                    if !menu_state.fen_history.contains(&fen) {
                        menu_state.fen_history.push(fen.clone());
                    }
                    menu_state.selected_fen = Some(fen);
                    menu_state.fen_dialog_state = None;
                } else {
                    dialog_state.validation_error = Some("Invalid FEN format".to_string());
                }
            } else {
                dialog_state.validation_error = Some("FEN cannot be empty".to_string());
            }
        }
        KeyCode::Tab => {
            // Switch focus between input and history
            dialog_state.focus = match dialog_state.focus {
                FenDialogFocus::Input => FenDialogFocus::HistoryList,
                FenDialogFocus::HistoryList => FenDialogFocus::Input,
            };
        }
        KeyCode::Char(c) => {
            // Add character to input buffer
            if dialog_state.focus == FenDialogFocus::Input {
                dialog_state.input_buffer.push(c);
                dialog_state.validation_error = None;
            }
        }
        KeyCode::Backspace => {
            // Remove last character
            if dialog_state.focus == FenDialogFocus::Input {
                dialog_state.input_buffer.pop();
                dialog_state.validation_error = None;
            }
        }
        KeyCode::Up => {
            // Navigate history list
            if dialog_state.focus == FenDialogFocus::HistoryList {
                if dialog_state.selected_history_index > 0 {
                    dialog_state.selected_history_index -= 1;
                }
            }
        }
        KeyCode::Down => {
            // Navigate history list
            if dialog_state.focus == FenDialogFocus::HistoryList {
                let max_index = menu_state.fen_history.len().saturating_sub(1);
                if dialog_state.selected_history_index < max_index {
                    dialog_state.selected_history_index += 1;
                }
            }
        }
        KeyCode::Right if dialog_state.focus == FenDialogFocus::HistoryList => {
            // Select from history
            if let Some(fen) = menu_state.fen_history.get(dialog_state.selected_history_index) {
                dialog_state.input_buffer = fen.clone();
                dialog_state.focus = FenDialogFocus::Input;
                dialog_state.validation_error = None;
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

    GameConfig {
        mode,
        skill_level,
        start_fen,
        time_control_seconds,
        resume: false,
    }
}
