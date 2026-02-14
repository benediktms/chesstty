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
}

/// Show menu and get game configuration
pub async fn show_menu() -> anyhow::Result<Option<GameConfig>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut menu_state = MenuState::default();
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
                match key.code {
                    KeyCode::Up => {
                        if menu_state.selected_index > 0 {
                            menu_state.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let max_index = 5; // 0-3: options, 4: Start Game, 5: Quit
                        if menu_state.selected_index < max_index {
                            menu_state.selected_index += 1;
                        }
                    }
                    KeyCode::Left => {
                        cycle_option(&mut menu_state, -1);
                    }
                    KeyCode::Right => {
                        cycle_option(&mut menu_state, 1);
                    }
                    KeyCode::Enter => {
                        match menu_state.selected_index {
                            4 => {
                                // Start Game selected
                                // Check if CustomFen is selected but no FEN provided
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen
                                    && menu_state.selected_fen.is_none()
                                {
                                    // Open FEN dialog
                                    menu_state.fen_dialog_state = Some(FenDialogState::new());
                                } else {
                                    // Start game with selected configuration
                                    let config = create_game_config(&menu_state);
                                    break Some(config);
                                }
                            }
                            5 => {
                                // Quit selected
                                break None;
                            }
                            3 => {
                                // Start Position - pressing Enter opens FEN dialog if CustomFen
                                use crate::ui::widgets::menu::StartPositionOption;
                                if menu_state.start_position == StartPositionOption::CustomFen {
                                    menu_state.fen_dialog_state = Some(FenDialogState::new());
                                }
                            }
                            _ => {
                                // Other rows - Enter does nothing
                            }
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break None;
                    }
                    KeyCode::Char(' ') => {
                        // Space on Start Position row opens FEN dialog
                        use crate::ui::widgets::menu::StartPositionOption;
                        if menu_state.selected_index == 3
                            && menu_state.start_position == StartPositionOption::CustomFen
                        {
                            menu_state.fen_dialog_state = Some(FenDialogState::new());
                        }
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

fn cycle_option(menu_state: &mut MenuState, direction: i32) {
    use crate::ui::widgets::menu::{DifficultyOption, GameModeOption, TimeControlOption, StartPositionOption};

    match menu_state.selected_index {
        0 => {
            // Game mode
            menu_state.game_mode = match menu_state.game_mode {
                GameModeOption::HumanVsHuman => {
                    if direction > 0 {
                        GameModeOption::HumanVsEngine
                    } else {
                        GameModeOption::EngineVsEngine
                    }
                }
                GameModeOption::HumanVsEngine => {
                    if direction > 0 {
                        GameModeOption::EngineVsEngine
                    } else {
                        GameModeOption::HumanVsHuman
                    }
                }
                GameModeOption::EngineVsEngine => {
                    if direction > 0 {
                        GameModeOption::HumanVsHuman
                    } else {
                        GameModeOption::HumanVsEngine
                    }
                }
            };
        }
        1 => {
            // Difficulty
            menu_state.difficulty = match menu_state.difficulty {
                DifficultyOption::Beginner => {
                    if direction > 0 {
                        DifficultyOption::Intermediate
                    } else {
                        DifficultyOption::Master
                    }
                }
                DifficultyOption::Intermediate => {
                    if direction > 0 {
                        DifficultyOption::Advanced
                    } else {
                        DifficultyOption::Beginner
                    }
                }
                DifficultyOption::Advanced => {
                    if direction > 0 {
                        DifficultyOption::Master
                    } else {
                        DifficultyOption::Intermediate
                    }
                }
                DifficultyOption::Master => {
                    if direction > 0 {
                        DifficultyOption::Beginner
                    } else {
                        DifficultyOption::Advanced
                    }
                }
            };
        }
        2 => {
            // Time control
            menu_state.time_control = match menu_state.time_control {
                TimeControlOption::None => {
                    if direction > 0 {
                        TimeControlOption::Blitz
                    } else {
                        TimeControlOption::Classical
                    }
                }
                TimeControlOption::Blitz => {
                    if direction > 0 {
                        TimeControlOption::Rapid
                    } else {
                        TimeControlOption::None
                    }
                }
                TimeControlOption::Rapid => {
                    if direction > 0 {
                        TimeControlOption::Classical
                    } else {
                        TimeControlOption::Blitz
                    }
                }
                TimeControlOption::Classical => {
                    if direction > 0 {
                        TimeControlOption::None
                    } else {
                        TimeControlOption::Rapid
                    }
                }
            };
        }
        3 => {
            // Start position
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
    use crate::ui::widgets::menu::{DifficultyOption, GameModeOption};

    let mode = match menu_state.game_mode {
        GameModeOption::HumanVsHuman => GameMode::HumanVsHuman,
        GameModeOption::HumanVsEngine => GameMode::HumanVsEngine {
            human_side: PlayerColor::White,
        },
        GameModeOption::EngineVsEngine => GameMode::EngineVsEngine,
    };

    let skill_level = match menu_state.difficulty {
        DifficultyOption::Beginner => 3,
        DifficultyOption::Intermediate => 10,
        DifficultyOption::Advanced => 15,
        DifficultyOption::Master => 20,
    };

    let start_fen = menu_state.selected_fen.clone();

    GameConfig {
        mode,
        skill_level,
        start_fen,
    }
}
