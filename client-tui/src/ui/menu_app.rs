use crate::state::{GameMode, PlayerColor};
use crate::ui::theme::Theme;
use crate::ui::widgets::selectable_table::SelectableTableState;
use crate::ui::widgets::{
    render_table_overlay, FenDialogState, FenDialogWidget, MenuState, MenuWidget,
    TableOverlayParams,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Constraint, Terminal};
use std::io;
use std::time::Duration;

#[derive(Clone, Debug)]
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
    pub resume_game_mode: Option<i32>,
    pub resume_human_side: Option<i32>,
    pub resume_skill_level: Option<u8>,
    /// If set, enter review mode with this pre-fetched review data.
    pub review_data: Option<chess_client::GameReviewProto>,
    /// Original game mode from a finished game (for snapshot creation during review).
    pub review_game_mode: Option<chess_client::GameModeProto>,
    /// Original skill level from a finished game (for snapshot creation during review).
    pub review_skill_level: Option<u8>,
    /// Pre-history moves from a snapshot (moves played before the snapshot position).
    /// `Some(...)` indicates a snapshot game, which implies paused start for engine modes.
    pub pre_history: Option<Vec<chess_client::MoveRecord>>,
    /// Advanced analysis data (tactical patterns, king safety, tension, psychological profiles).
    pub advanced_data: Option<chess_client::AdvancedGameAnalysisProto>,
}

/// Actions returned from the menu.
pub enum MenuAction {
    /// Start a game (or review) with the given config.
    StartGame(Box<GameConfig>),
    /// Enqueue a game for review analysis, then return to menu.
    EnqueueReview(String),
    /// User chose to quit.
    Quit,
}

/// Show menu and get game configuration.
/// Pre-fetched data from the server is passed in to avoid async calls during menu rendering.
pub async fn show_menu(
    suspended_sessions: Vec<chess_client::SuspendedSessionInfo>,
    saved_positions: Vec<chess_client::SavedPosition>,
    finished_games: Vec<chess_client::FinishedGameInfo>,
) -> anyhow::Result<MenuAction> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let has_saved_session = !suspended_sessions.is_empty();
    let has_finished_games = !finished_games.is_empty();

    let mut menu_state = MenuState {
        suspended_sessions,
        saved_positions,
        has_saved_session,
        has_finished_games,
        finished_games,
        ..Default::default()
    };

    let theme = Theme::default();

    let result = loop {
        terminal.draw(|f| {
            let menu_widget = MenuWidget {
                menu_state: &menu_state,
                theme: &theme,
            };
            f.render_widget(menu_widget, f.area());

            // Render FEN dialog if active
            if let Some(ref mut dialog_state) = menu_state.fen_dialog_state {
                let fen_dialog = FenDialogWidget::new(dialog_state, &menu_state.saved_positions, &theme);
                f.render_widget(fen_dialog, f.area());
            }

            // Render review game selection table if active
            if let Some(ref mut ctx) = menu_state.review_table {
                let rows: Vec<Vec<String>> = ctx
                    .games
                    .iter()
                    .map(|g| {
                        let result = &g.result;
                        let reason = if g.result_reason.len() > 15 {
                            format!("{}...", &g.result_reason[..12])
                        } else {
                            g.result_reason.clone()
                        };
                        let moves = format!("{} moves", g.move_count);
                        let status = g
                            .review_status
                            .and_then(|s| chess_client::ReviewStatusType::try_from(s).ok())
                            .map(|s| match s {
                                chess_client::ReviewStatusType::ReviewStatusQueued => "Queued",
                                chess_client::ReviewStatusType::ReviewStatusAnalyzing => {
                                    "Analyzing"
                                }
                                chess_client::ReviewStatusType::ReviewStatusComplete => "Reviewed",
                                chess_client::ReviewStatusType::ReviewStatusFailed => "Failed",
                            })
                            .unwrap_or("Not reviewed");
                        vec![result.clone(), reason, moves, status.to_string()]
                    })
                    .collect();

                render_table_overlay(
                    f.area(),
                    f.buffer_mut(),
                    TableOverlayParams {
                        title: "Review Game",
                        headers: &["Result", "Reason", "Moves", "Status"],
                        rows: &rows,
                        column_widths: &[
                            Constraint::Length(12),
                            Constraint::Length(16),
                            Constraint::Length(10),
                            Constraint::Length(14),
                        ],
                        state: &mut ctx.table_state,
                        width: 65,
                        height: (ctx.games.len() as u16 + 6).min(20),
                        footer: Some("Enter: View reviewed | a: Analyze | Esc: Back"),
                        theme: &theme,
                    },
                );
            }

            // Render session selection table if active
            if let Some(ref mut ctx) = menu_state.session_table {
                let rows: Vec<Vec<String>> = ctx
                    .sessions
                    .iter()
                    .map(|s| {
                        let mode = s
                            .game_mode
                            .as_ref()
                            .and_then(|gm| chess_client::GameModeType::try_from(gm.mode).ok())
                            .map(|t| match t {
                                chess_client::GameModeType::HumanVsHuman => "HumanVsHuman",
                                chess_client::GameModeType::HumanVsEngine => "HumanVsEngine",
                                chess_client::GameModeType::EngineVsEngine => "EngineVsEngine",
                                chess_client::GameModeType::Analysis => "Analysis",
                                chess_client::GameModeType::Review => "Review",
                            })
                            .unwrap_or("Unknown");
                        let moves = format!("{} moves", s.move_count);
                        let side = s.side_to_move.clone();
                        let fen_preview = if s.fen.len() > 30 {
                            format!("{}...", &s.fen[..27])
                        } else {
                            s.fen.clone()
                        };
                        vec![mode.to_string(), moves, side, fen_preview]
                    })
                    .collect();

                render_table_overlay(
                    f.area(),
                    f.buffer_mut(),
                    TableOverlayParams {
                        title: "Resume Session",
                        headers: &["Mode", "Moves", "Turn", "Position"],
                        rows: &rows,
                        column_widths: &[
                            Constraint::Length(16),
                            Constraint::Length(10),
                            Constraint::Length(8),
                            Constraint::Min(20),
                        ],
                        state: &mut ctx.table_state,
                        width: 70,
                        height: (ctx.sessions.len() as u16 + 5).min(20),
                        footer: None,
                        theme: &theme,
                    },
                );
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Review table takes highest priority
                if menu_state.review_table.is_some() {
                    let action = handle_review_table_input(&mut menu_state, key.code);
                    if let Some(menu_action) = action {
                        break menu_action;
                    }
                    continue;
                }

                // Session table takes next priority
                if menu_state.session_table.is_some() {
                    let action = handle_session_table_input(&mut menu_state, key.code);
                    if let Some(config) = action {
                        break MenuAction::StartGame(Box::new(config));
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
                    KeyCode::Enter => match selected_item {
                        Some(MenuItem::StartGame) => {
                            use crate::ui::widgets::menu::StartPositionOption;
                            if menu_state.start_position == StartPositionOption::CustomFen
                                && menu_state.selected_fen.is_none()
                            {
                                menu_state.fen_dialog_state =
                                    Some(FenDialogState::new(menu_state.saved_positions.len()));
                            } else {
                                let config = create_game_config(&menu_state);
                                break MenuAction::StartGame(Box::new(config));
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
                        Some(MenuItem::ReviewGame) => {
                            let games = menu_state.finished_games.clone();
                            if !games.is_empty() {
                                use crate::ui::widgets::menu::ReviewTableContext;
                                let count = games.len();
                                menu_state.review_table = Some(ReviewTableContext {
                                    table_state: SelectableTableState::new(count),
                                    games,
                                });
                            }
                        }
                        Some(MenuItem::Quit) => {
                            break MenuAction::Quit;
                        }
                        Some(MenuItem::StartPosition(_)) => {
                            use crate::ui::widgets::menu::StartPositionOption;
                            if menu_state.start_position == StartPositionOption::CustomFen {
                                menu_state.fen_dialog_state =
                                    Some(FenDialogState::new(menu_state.saved_positions.len()));
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break MenuAction::Quit;
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

fn cycle_option(
    menu_state: &mut MenuState,
    selected_item: &Option<crate::ui::widgets::menu::MenuItem>,
    _direction: i32,
) {
    use crate::ui::widgets::menu::{
        DifficultyOption, GameModeOption, MenuItem, StartPositionOption, TimeControlOption,
    };

    let Some(item) = selected_item else { return };

    match item {
        MenuItem::GameMode(_) => {
            menu_state.game_mode = match menu_state.game_mode {
                GameModeOption::HumanVsHuman => {
                    if _direction > 0 {
                        GameModeOption::HumanVsEngine
                    } else {
                        GameModeOption::EngineVsEngine
                    }
                }
                GameModeOption::HumanVsEngine => {
                    if _direction > 0 {
                        GameModeOption::EngineVsEngine
                    } else {
                        GameModeOption::HumanVsHuman
                    }
                }
                GameModeOption::EngineVsEngine => {
                    if _direction > 0 {
                        GameModeOption::HumanVsHuman
                    } else {
                        GameModeOption::HumanVsEngine
                    }
                }
            };
        }
        MenuItem::PlayAs(_) => {
            menu_state.cycle_play_as();
        }
        MenuItem::Difficulty(_) => {
            menu_state.difficulty = match menu_state.difficulty {
                DifficultyOption::Beginner => {
                    if _direction > 0 {
                        DifficultyOption::Intermediate
                    } else {
                        DifficultyOption::Master
                    }
                }
                DifficultyOption::Intermediate => {
                    if _direction > 0 {
                        DifficultyOption::Advanced
                    } else {
                        DifficultyOption::Beginner
                    }
                }
                DifficultyOption::Advanced => {
                    if _direction > 0 {
                        DifficultyOption::Master
                    } else {
                        DifficultyOption::Intermediate
                    }
                }
                DifficultyOption::Master => {
                    if _direction > 0 {
                        DifficultyOption::Beginner
                    } else {
                        DifficultyOption::Advanced
                    }
                }
            };
        }
        MenuItem::TimeControl(_) => {
            menu_state.time_control = match menu_state.time_control {
                TimeControlOption::None => {
                    if _direction > 0 {
                        TimeControlOption::Blitz
                    } else {
                        TimeControlOption::Classical
                    }
                }
                TimeControlOption::Blitz => {
                    if _direction > 0 {
                        TimeControlOption::Rapid
                    } else {
                        TimeControlOption::None
                    }
                }
                TimeControlOption::Rapid => {
                    if _direction > 0 {
                        TimeControlOption::Classical
                    } else {
                        TimeControlOption::Blitz
                    }
                }
                TimeControlOption::Classical => {
                    if _direction > 0 {
                        TimeControlOption::None
                    } else {
                        TimeControlOption::Rapid
                    }
                }
            };
        }
        MenuItem::EngineThreads(_) => {
            use crate::ui::widgets::menu::ThreadsOption;
            menu_state.engine_threads = match menu_state.engine_threads {
                ThreadsOption::Auto => {
                    if _direction > 0 {
                        ThreadsOption::One
                    } else {
                        ThreadsOption::Four
                    }
                }
                ThreadsOption::One => {
                    if _direction > 0 {
                        ThreadsOption::Two
                    } else {
                        ThreadsOption::Auto
                    }
                }
                ThreadsOption::Two => {
                    if _direction > 0 {
                        ThreadsOption::Four
                    } else {
                        ThreadsOption::One
                    }
                }
                ThreadsOption::Four => {
                    if _direction > 0 {
                        ThreadsOption::Auto
                    } else {
                        ThreadsOption::Two
                    }
                }
            };
        }
        MenuItem::EngineHash(_) => {
            use crate::ui::widgets::menu::HashOption;
            menu_state.engine_hash = match menu_state.engine_hash {
                HashOption::Small => {
                    if _direction > 0 {
                        HashOption::Medium
                    } else {
                        HashOption::Large
                    }
                }
                HashOption::Medium => {
                    if _direction > 0 {
                        HashOption::Large
                    } else {
                        HashOption::Small
                    }
                }
                HashOption::Large => {
                    if _direction > 0 {
                        HashOption::Small
                    } else {
                        HashOption::Medium
                    }
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
                                    dialog_state
                                        .position_table
                                        .update_row_count(menu_state.saved_positions.len());
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
        review_data: None,
        review_game_mode: None,
        review_skill_level: None,
        pre_history: None,
        advanced_data: None,
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
                    config.resume_game_mode = session.game_mode.as_ref().map(|gm| gm.mode);
                    config.resume_human_side =
                        session.game_mode.as_ref().and_then(|gm| gm.human_side);
                    config.resume_skill_level = None;
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

/// Handle input for the review game selection table.
/// Returns Some(MenuAction) when user picks a game or enqueues analysis.
fn handle_review_table_input(menu_state: &mut MenuState, key_code: KeyCode) -> Option<MenuAction> {
    let ctx = menu_state.review_table.as_mut()?;

    match key_code {
        KeyCode::Up | KeyCode::Char('k') => {
            ctx.table_state.move_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            ctx.table_state.move_down();
        }
        KeyCode::Enter => {
            if let Some(idx) = ctx.table_state.selected_index() {
                if let Some(game) = ctx.games.get(idx) {
                    let status = game
                        .review_status
                        .and_then(|s| chess_client::ReviewStatusType::try_from(s).ok());

                    if status == Some(chess_client::ReviewStatusType::ReviewStatusComplete) {
                        let game_id = game.game_id.clone();
                        let review_game_mode = game.game_mode;
                        menu_state.review_table = None;
                        return Some(MenuAction::StartGame(Box::new(GameConfig {
                            mode: GameMode::ReviewMode,
                            skill_level: 0,
                            start_fen: None,
                            time_control_seconds: None,
                            engine_threads: None,
                            engine_hash_mb: None,
                            resume_session_id: Some(game_id),
                            resume_game_mode: None,
                            resume_human_side: None,
                            resume_skill_level: None,
                            review_data: None,
                            review_game_mode,
                            review_skill_level: None,
                            pre_history: None,
                            advanced_data: None,
                        })));
                    }
                }
            }
        }
        KeyCode::Char('a') => {
            // Enqueue analysis for the selected game (only if not reviewed and not in-flight)
            if let Some(idx) = ctx.table_state.selected_index() {
                if let Some(game) = ctx.games.get(idx) {
                    let status = game
                        .review_status
                        .and_then(|s| chess_client::ReviewStatusType::try_from(s).ok());
                    let can_enqueue = matches!(
                        status,
                        None | Some(chess_client::ReviewStatusType::ReviewStatusFailed)
                    );
                    if can_enqueue {
                        let game_id = game.game_id.clone();
                        menu_state.review_table = None;
                        return Some(MenuAction::EnqueueReview(game_id));
                    }
                }
            }
        }
        KeyCode::Esc => {
            menu_state.review_table = None;
        }
        _ => {}
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::menu::ReviewTableContext;

    fn sample_game(game_id: &str, review_status: Option<i32>) -> chess_client::FinishedGameInfo {
        chess_client::FinishedGameInfo {
            game_id: game_id.to_string(),
            result: "BlackWins".to_string(),
            result_reason: "Checkmate".to_string(),
            game_mode: None,
            move_count: 4,
            created_at: 1000,
            review_status,
        }
    }

    fn menu_with_review_table(games: Vec<chess_client::FinishedGameInfo>) -> MenuState {
        let count = games.len();
        let mut state = MenuState {
            has_finished_games: !games.is_empty(),
            finished_games: games.clone(),
            ..Default::default()
        };
        state.review_table = Some(ReviewTableContext {
            table_state: SelectableTableState::new(count),
            games,
        });
        state
    }

    #[test]
    fn test_enter_on_reviewed_game_returns_start_game() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusComplete as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Enter);
        assert!(action.is_some());
        match action.unwrap() {
            MenuAction::StartGame(config) => {
                assert_eq!(config.mode, GameMode::ReviewMode);
                assert_eq!(config.resume_session_id, Some("game_1".to_string()));
            }
            _ => panic!("Expected StartGame"),
        }
        // Table should be dismissed
        assert!(state.review_table.is_none());
    }

    #[test]
    fn test_enter_on_unreviewed_game_does_nothing() {
        let games = vec![sample_game("game_1", None)];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Enter);
        assert!(action.is_none());
        // Table should remain open
        assert!(state.review_table.is_some());
    }

    #[test]
    fn test_enter_on_analyzing_game_does_nothing() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusAnalyzing as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Enter);
        assert!(action.is_none());
        assert!(state.review_table.is_some());
    }

    #[test]
    fn test_a_on_unreviewed_game_enqueues() {
        let games = vec![sample_game("game_1", None)];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Char('a'));
        assert!(action.is_some());
        match action.unwrap() {
            MenuAction::EnqueueReview(id) => assert_eq!(id, "game_1"),
            _ => panic!("Expected EnqueueReview"),
        }
        assert!(state.review_table.is_none());
    }

    #[test]
    fn test_a_on_failed_game_enqueues() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusFailed as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Char('a'));
        assert!(action.is_some());
        match action.unwrap() {
            MenuAction::EnqueueReview(id) => assert_eq!(id, "game_1"),
            _ => panic!("Expected EnqueueReview"),
        }
    }

    #[test]
    fn test_a_on_completed_review_does_nothing() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusComplete as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Char('a'));
        assert!(action.is_none());
        assert!(state.review_table.is_some());
    }

    #[test]
    fn test_a_on_queued_game_does_nothing() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusQueued as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Char('a'));
        assert!(action.is_none());
        assert!(state.review_table.is_some());
    }

    #[test]
    fn test_a_on_analyzing_game_does_nothing() {
        let games = vec![sample_game(
            "game_1",
            Some(chess_client::ReviewStatusType::ReviewStatusAnalyzing as i32),
        )];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Char('a'));
        assert!(action.is_none());
        assert!(state.review_table.is_some());
    }

    #[test]
    fn test_esc_closes_review_table() {
        let games = vec![sample_game("game_1", None)];
        let mut state = menu_with_review_table(games);

        let action = handle_review_table_input(&mut state, KeyCode::Esc);
        assert!(action.is_none());
        assert!(state.review_table.is_none());
    }

    #[test]
    fn test_navigation_in_review_table() {
        let games = vec![
            sample_game("game_1", None),
            sample_game("game_2", None),
            sample_game("game_3", None),
        ];
        let mut state = menu_with_review_table(games);

        // Starts at index 0
        let ctx = state.review_table.as_ref().unwrap();
        assert_eq!(ctx.table_state.selected_index(), Some(0));

        // Move down
        handle_review_table_input(&mut state, KeyCode::Down);
        let ctx = state.review_table.as_ref().unwrap();
        assert_eq!(ctx.table_state.selected_index(), Some(1));

        // Move down with 'j'
        handle_review_table_input(&mut state, KeyCode::Char('j'));
        let ctx = state.review_table.as_ref().unwrap();
        assert_eq!(ctx.table_state.selected_index(), Some(2));

        // Move up with 'k'
        handle_review_table_input(&mut state, KeyCode::Char('k'));
        let ctx = state.review_table.as_ref().unwrap();
        assert_eq!(ctx.table_state.selected_index(), Some(1));

        // Move up
        handle_review_table_input(&mut state, KeyCode::Up);
        let ctx = state.review_table.as_ref().unwrap();
        assert_eq!(ctx.table_state.selected_index(), Some(0));
    }

    #[test]
    fn test_enter_selects_correct_game_after_navigation() {
        let games = vec![
            sample_game("game_1", None),
            sample_game(
                "game_2",
                Some(chess_client::ReviewStatusType::ReviewStatusComplete as i32),
            ),
        ];
        let mut state = menu_with_review_table(games);

        // Navigate to second game
        handle_review_table_input(&mut state, KeyCode::Down);

        let action = handle_review_table_input(&mut state, KeyCode::Enter);
        match action.unwrap() {
            MenuAction::StartGame(config) => {
                assert_eq!(config.resume_session_id, Some("game_2".to_string()));
            }
            _ => panic!("Expected StartGame"),
        }
    }
}
