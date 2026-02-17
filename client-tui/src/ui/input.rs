use crate::state::{ClientState, GameMode, InputPhase};
use crate::ui::context::FocusContext;
use crate::ui::menu_app::GameConfig;
use crate::ui::pane::PaneId;
use crate::ui::widgets::popup_menu::{PopupMenuItem, PopupMenuState};
use crate::ui::widgets::snapshot_dialog::{SnapshotDialogFocus, SnapshotDialogState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions returned from key handling that the main loop must process.
pub enum AppAction {
    /// Continue the game loop normally.
    Continue,
    /// Quit the application.
    Quit,
    /// Return to the main menu.
    ReturnToMenu,
    /// Suspend session (save state) then return to menu.
    SuspendAndReturnToMenu,
    /// Play from a snapshot — exit review and start a new game with the given config.
    PlaySnapshot(Box<GameConfig>),
}

/// Returns true if character input should be disabled for the given game mode.
pub fn should_disable_input(mode: &GameMode) -> bool {
    matches!(mode, GameMode::EngineVsEngine | GameMode::ReviewMode)
}

/// Main key dispatch function. Routes input to the appropriate context handler.
pub async fn handle_key(
    state: &mut ClientState,
    input_buffer: &mut String,
    key: KeyEvent,
) -> AppAction {
    // Tab input mode takes priority (modal overlay)
    if state.ui.tab_input.active {
        return handle_tab_input(state, key).await;
    }

    // Popup menu takes highest priority (modal overlay)
    if state.ui.popup_menu.is_some() {
        return handle_popup_input(state, key).await;
    }

    // Snapshot dialog takes priority (modal overlay)
    if state.ui.snapshot_dialog.is_some() {
        return handle_snapshot_dialog_input(state, key).await;
    }

    // Promotion dialog takes priority (modal overlay)
    if matches!(state.ui.input_phase, InputPhase::SelectPromotion { .. }) {
        return handle_promotion_input(state, input_buffer, key);
    }

    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return AppAction::Quit;
    }

    // Global toggles that work in any context
    match key.code {
        KeyCode::Char('@') => {
            state.ui.pane_manager.toggle_visibility(PaneId::UciDebug);
            return AppAction::Continue;
        }
        KeyCode::Char('#') => {
            state
                .ui
                .pane_manager
                .toggle_visibility(PaneId::EngineAnalysis);
            return AppAction::Continue;
        }
        _ => {}
    }

    // Dispatch by context
    let context = state.ui.focus_stack.current().clone();
    match context {
        FocusContext::Board => handle_board_context(state, input_buffer, key).await,
        FocusContext::PaneSelected { pane_id } => handle_pane_selected_context(state, pane_id, key),
        FocusContext::PaneExpanded { pane_id } => handle_pane_expanded_context(state, pane_id, key),
    }
}

/// Handle keys in Board context (default — user is interacting with the chess board).
async fn handle_board_context(
    state: &mut ClientState,
    input_buffer: &mut String,
    key: KeyEvent,
) -> AppAction {
    // Review mode: navigation keys instead of move input
    if matches!(state.mode, GameMode::ReviewMode) {
        if let Some(ref mut review) = state.review_state {
            match key.code {
                KeyCode::Right | KeyCode::Char('l') => {
                    review.next_ply();
                    return AppAction::Continue;
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    review.prev_ply();
                    return AppAction::Continue;
                }
                KeyCode::Home => {
                    review.go_to_start();
                    return AppAction::Continue;
                }
                KeyCode::End => {
                    review.go_to_end();
                    return AppAction::Continue;
                }
                KeyCode::Char('n') => {
                    let current = review.current_ply;
                    if let Some(&next) = review.critical_moments().iter().find(|&&p| p > current) {
                        review.go_to_ply(next);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char('p') => {
                    let current = review.current_ply;
                    if let Some(&prev) = review
                        .critical_moments()
                        .iter()
                        .rev()
                        .find(|&&p| p < current)
                    {
                        review.go_to_ply(prev);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char(' ') => {
                    review.auto_play = !review.auto_play;
                    return AppAction::Continue;
                }
                KeyCode::Tab => {
                    if let Some(first) = state.ui.pane_manager.first_selectable() {
                        state
                            .ui
                            .focus_stack
                            .push(FocusContext::PaneSelected { pane_id: first });
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char('s') => {
                    // Open snapshot dialog
                    let current_ply = review.current_ply;
                    let game_id = review.review.game_id.clone();
                    let positions = &review.review.positions;
                    state.ui.snapshot_dialog =
                        Some(SnapshotDialogState::new(current_ply, &game_id, positions));
                    return AppAction::Continue;
                }
                KeyCode::Esc => {
                    state.ui.popup_menu = Some(PopupMenuState::new(&state.mode));
                    return AppAction::Continue;
                }
                _ => return AppAction::Continue,
            }
        }
    }

    match key.code {
        // Tab input mode activation
        KeyCode::Char('i') if !should_disable_input(&state.mode) => {
            state.ui.tab_input.activate();
            return AppAction::Continue;
        }
        KeyCode::Tab => {
            // Enter pane selection mode
            if let Some(first) = state.ui.pane_manager.first_selectable() {
                state
                    .ui
                    .focus_stack
                    .push(FocusContext::PaneSelected { pane_id: first });
            }
        }
        // Pause toggle (any engine mode) — must be before Char(c) catch-all
        KeyCode::Char('p')
            if matches!(
                state.mode,
                GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
            ) =>
        {
            if state.ui.paused {
                match state.client.resume().await {
                    Ok(()) => {
                        state.ui.paused = false;
                        state.ui.status_message = Some("Playing".to_string());
                    }
                    Err(e) => {
                        state.ui.status_message = Some(format!("Resume error: {}", e));
                    }
                }
            } else {
                match state.client.pause().await {
                    Ok(()) => {
                        state.ui.paused = true;
                        state.ui.status_message = Some("Paused".to_string());
                    }
                    Err(e) => {
                        state.ui.status_message = Some(format!("Pause error: {}", e));
                    }
                }
            }
        }
        KeyCode::Char(c) => {
            if !should_disable_input(&state.mode) {
                input_buffer.push(c);
            }
        }
        KeyCode::Backspace => {
            input_buffer.pop();
        }
        KeyCode::Enter => {
            if !input_buffer.is_empty() {
                super::full_ui::handle_input(state, input_buffer).await;
                input_buffer.clear();
            }
        }
        KeyCode::Esc => {
            if state.ui.selected_square.is_some() {
                state.clear_selection();
                input_buffer.clear();
            } else {
                input_buffer.clear();
                // Auto-pause on server when opening menu (any mode with an engine)
                let has_engine = matches!(
                    state.mode,
                    GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
                );
                if has_engine {
                    state.ui.paused_before_menu = state.ui.paused;
                    if !state.ui.paused {
                        let _ = state.client.pause().await;
                        state.ui.paused = true;
                        state.ui.status_message = Some("Paused".to_string());
                    }
                }
                state.ui.popup_menu = Some(PopupMenuState::new(&state.mode));
            }
        }
        _ => {}
    }
    AppAction::Continue
}

/// Restore pause state after popup menu is dismissed.
async fn restore_pause_state(state: &mut ClientState) {
    let has_engine = matches!(
        state.mode,
        GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
    );
    if has_engine && !state.ui.paused_before_menu && state.ui.paused {
        // Was not paused before menu — resume on server
        let _ = state.client.resume().await;
        state.ui.paused = false;
    }
}

/// Handle keys when the popup menu is active.
async fn handle_popup_input(state: &mut ClientState, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut menu) = state.ui.popup_menu {
                menu.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut menu) = state.ui.popup_menu {
                menu.move_down();
            }
        }
        KeyCode::Enter => {
            let selected = state
                .ui
                .popup_menu
                .as_ref()
                .map(|m| m.selected_item().clone());

            state.ui.popup_menu = None;
            restore_pause_state(state).await;

            if let Some(item) = selected {
                match item {
                    PopupMenuItem::Restart => {
                        if let Err(e) = state.reset(None).await {
                            state.ui.status_message = Some(format!("Reset error: {}", e));
                        }
                    }
                    PopupMenuItem::AdjustDifficulty => {
                        let new_level = match state.skill_level {
                            0..=5 => 10,
                            6..=12 => 15,
                            13..=18 => 20,
                            _ => 3,
                        };
                        if let Err(e) = state.set_engine(true, new_level).await {
                            state.ui.status_message = Some(format!("Engine error: {}", e));
                        } else {
                            let label = match new_level {
                                3 => "Beginner",
                                10 => "Intermediate",
                                15 => "Advanced",
                                20 => "Master",
                                _ => "Custom",
                            };
                            state.ui.status_message = Some(format!("Difficulty set to {}", label));
                        }
                    }
                    PopupMenuItem::SuspendSession => {
                        return AppAction::SuspendAndReturnToMenu;
                    }
                    PopupMenuItem::Quit => {
                        return AppAction::ReturnToMenu;
                    }
                }
            }
        }
        KeyCode::Esc => {
            state.ui.popup_menu = None;
            restore_pause_state(state).await;
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys when the snapshot dialog is active (modal overlay).
async fn handle_snapshot_dialog_input(state: &mut ClientState, key: KeyEvent) -> AppAction {
    // Get positions slice for terminal checks during navigation
    let positions: Vec<_> = state
        .review_state
        .as_ref()
        .map(|rs| rs.review.positions.clone())
        .unwrap_or_default();

    let dialog = match state.ui.snapshot_dialog.as_mut() {
        Some(d) => d,
        None => return AppAction::Continue,
    };

    match key.code {
        KeyCode::Esc => {
            state.ui.snapshot_dialog = None;
        }
        KeyCode::Tab => {
            dialog.next_focus();
        }
        KeyCode::Char('j') if dialog.focus != SnapshotDialogFocus::Name => {
            dialog.next_focus();
        }
        KeyCode::Char('k') if dialog.focus != SnapshotDialogFocus::Name => {
            dialog.prev_focus();
        }
        KeyCode::Left | KeyCode::Char('h') if dialog.focus == SnapshotDialogFocus::MovesBack => {
            dialog.decrement_moves_back(&positions);
        }
        KeyCode::Right | KeyCode::Char('l') if dialog.focus == SnapshotDialogFocus::MovesBack => {
            dialog.increment_moves_back(&positions);
        }
        KeyCode::Left | KeyCode::Char('h') if dialog.focus == SnapshotDialogFocus::PlayNow => {
            dialog.play_immediately = true;
        }
        KeyCode::Right | KeyCode::Char('l') if dialog.focus == SnapshotDialogFocus::PlayNow => {
            dialog.play_immediately = false;
        }
        KeyCode::Char(c) if dialog.focus == SnapshotDialogFocus::Name => {
            dialog.name_buffer.push(c);
        }
        KeyCode::Backspace if dialog.focus == SnapshotDialogFocus::Name => {
            dialog.name_buffer.pop();
        }
        KeyCode::Enter => {
            // Block confirm if target position is terminal
            if dialog.is_target_terminal {
                state.ui.status_message =
                    Some("Cannot create snapshot at a terminal position".to_string());
                return AppAction::Continue;
            }

            // Confirm: extract dialog state and execute
            let dialog = state.ui.snapshot_dialog.take().unwrap();
            let target_ply = dialog.target_ply();

            // Get FEN at target ply from review state
            let review = match &state.review_state {
                Some(rs) => rs,
                None => return AppAction::Continue,
            };

            let fen = if target_ply == 0 {
                cozy_chess::Board::default().to_string()
            } else {
                review
                    .review
                    .positions
                    .iter()
                    .find(|p| p.ply == target_ply)
                    .map(|p| p.fen.clone())
                    .unwrap_or_else(|| cozy_chess::Board::default().to_string())
            };

            // Build pre-history from review positions up to target ply
            let pre_history: Vec<chess_client::MoveRecord> = review
                .move_history
                .iter()
                .take(target_ply as usize)
                .cloned()
                .collect();

            if dialog.play_immediately {
                // Build GameConfig for the new game
                let game_mode = review.game_mode;
                let skill_level = review.skill_level;

                // Determine local GameMode from proto
                let mode = game_mode
                    .as_ref()
                    .map(crate::state::game_mode_from_proto)
                    .unwrap_or(GameMode::HumanVsEngine {
                        human_side: crate::state::PlayerColor::White,
                    });

                let config = GameConfig {
                    mode,
                    skill_level,
                    start_fen: Some(fen),
                    time_control_seconds: None,
                    engine_threads: None,
                    engine_hash_mb: None,
                    resume_session_id: None,
                    resume_game_mode: None,
                    resume_human_side: None,
                    resume_skill_level: None,
                    review_data: None,
                    review_game_mode: None,
                    review_skill_level: None,
                    pre_history: Some(pre_history),
                };
                return AppAction::PlaySnapshot(Box::new(config));
            } else {
                // Save for later via RPC
                let game_mode = review.game_mode;
                let skill_level = review.skill_level;
                let move_count = target_ply;
                let name = dialog.effective_name();

                match state
                    .client
                    .save_snapshot(&fen, &name, game_mode, move_count, skill_level)
                    .await
                {
                    Ok(_) => {
                        state.ui.status_message = Some("Snapshot saved".to_string());
                    }
                    Err(e) => {
                        state.ui.status_message = Some(format!("Failed to save snapshot: {}", e));
                    }
                }
            }
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys in PaneSelected context (a pane is highlighted, user navigates/scrolls).
fn handle_pane_selected_context(
    state: &mut ClientState,
    pane_id: PaneId,
    key: KeyEvent,
) -> AppAction {
    // Forward review navigation keys (n/p/Space/Home/End) from pane context
    if matches!(state.mode, GameMode::ReviewMode) {
        if let Some(ref mut review) = state.review_state {
            match key.code {
                KeyCode::Char('n') => {
                    let current = review.current_ply;
                    if let Some(&next) = review.critical_moments().iter().find(|&&p| p > current) {
                        review.go_to_ply(next);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char('p') => {
                    let current = review.current_ply;
                    if let Some(&prev) = review
                        .critical_moments()
                        .iter()
                        .rev()
                        .find(|&&p| p < current)
                    {
                        review.go_to_ply(prev);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char(' ') => {
                    review.auto_play = !review.auto_play;
                    return AppAction::Continue;
                }
                KeyCode::Home => {
                    review.go_to_start();
                    return AppAction::Continue;
                }
                KeyCode::End => {
                    review.go_to_end();
                    return AppAction::Continue;
                }
                _ => {} // Fall through to normal pane handling
            }
        }
    }

    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(prev) = state.ui.pane_manager.prev_selectable(pane_id) {
                state.ui.focus_stack.pop();
                state
                    .ui
                    .focus_stack
                    .push(FocusContext::PaneSelected { pane_id: prev });
            }
        }
        KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => {
            if let Some(next) = state.ui.pane_manager.next_selectable(pane_id) {
                state.ui.focus_stack.pop();
                state
                    .ui
                    .focus_stack
                    .push(FocusContext::PaneSelected { pane_id: next });
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let scroll = state.ui.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_sub(5);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let scroll = state.ui.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_add(5);
        }
        KeyCode::PageUp => {
            *state.ui.pane_manager.scroll_mut(pane_id) = 0;
        }
        KeyCode::PageDown => {
            *state.ui.pane_manager.scroll_mut(pane_id) = u16::MAX;
        }
        KeyCode::Enter => {
            use crate::ui::pane::pane_properties;
            let props = pane_properties(pane_id);
            if props.is_expandable {
                state
                    .ui
                    .focus_stack
                    .push(FocusContext::PaneExpanded { pane_id });
            }
        }
        KeyCode::Esc => {
            state.ui.focus_stack.pop();
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys in PaneExpanded context (a pane fills the board area).
fn handle_pane_expanded_context(
    state: &mut ClientState,
    pane_id: PaneId,
    key: KeyEvent,
) -> AppAction {
    // Forward review navigation keys (n/p/Space/Home/End) from expanded pane
    if matches!(state.mode, GameMode::ReviewMode) {
        if let Some(ref mut review) = state.review_state {
            match key.code {
                KeyCode::Char('n') => {
                    let current = review.current_ply;
                    if let Some(&next) = review.critical_moments().iter().find(|&&p| p > current) {
                        review.go_to_ply(next);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char('p') => {
                    let current = review.current_ply;
                    if let Some(&prev) = review
                        .critical_moments()
                        .iter()
                        .rev()
                        .find(|&&p| p < current)
                    {
                        review.go_to_ply(prev);
                    }
                    return AppAction::Continue;
                }
                KeyCode::Char(' ') => {
                    review.auto_play = !review.auto_play;
                    return AppAction::Continue;
                }
                KeyCode::Home => {
                    review.go_to_start();
                    return AppAction::Continue;
                }
                KeyCode::End => {
                    review.go_to_end();
                    return AppAction::Continue;
                }
                _ => {} // Fall through to normal expanded handling
            }
        }
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            let scroll = state.ui.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_sub(5);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let scroll = state.ui.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_add(5);
        }
        KeyCode::PageUp => {
            *state.ui.pane_manager.scroll_mut(pane_id) = 0;
        }
        KeyCode::PageDown => {
            *state.ui.pane_manager.scroll_mut(pane_id) = u16::MAX;
        }
        KeyCode::Esc => {
            state.ui.focus_stack.pop();
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys when the promotion dialog is active (modal overlay).
fn handle_promotion_input(
    state: &mut ClientState,
    input_buffer: &mut String,
    key: KeyEvent,
) -> AppAction {
    match key.code {
        KeyCode::Char(c) => {
            input_buffer.push(c);
        }
        KeyCode::Enter => {
            // Handled through the normal input flow
        }
        KeyCode::Esc => {
            state.clear_selection();
            input_buffer.clear();
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys when tab input mode is active (modal overlay).
async fn handle_tab_input(state: &mut ClientState, key: KeyEvent) -> AppAction {
    use chess::parse_square;

    match key.code {
        KeyCode::Esc => {
            state.ui.tab_input.deactivate();
            state.clear_selection();
        }

        KeyCode::Backspace => {
            if state.ui.tab_input.typeahead_buffer.is_empty() {
                // In tab 2 with empty buffer: go back to tab 1
                if state.ui.tab_input.current_tab == 1 {
                    state.clear_selection();
                    state.ui.tab_input.current_tab = 0;
                    state.ui.tab_input.from_square = None;
                    state.ui.tab_input.typeahead_buffer.clear();
                }
            } else {
                state.ui.tab_input.typeahead_buffer.pop();
            }
        }

        KeyCode::Char(c) => {
            let buf_len = state.ui.tab_input.typeahead_buffer.len();
            // Restrict: first char a-h, second char 1-8, max 2
            let valid = match buf_len {
                0 => c.is_ascii_lowercase() && ('a'..='h').contains(&c),
                1 => c.is_ascii_digit() && ('1'..='8').contains(&c),
                _ => false,
            };
            if !valid {
                return AppAction::Continue;
            }

            state.ui.tab_input.typeahead_buffer.push(c);

            // Tab 1: auto-advance on valid 2-char piece square
            if state.ui.tab_input.current_tab == 0 && state.ui.tab_input.typeahead_buffer.len() == 2
            {
                if let Some(from_square) = parse_square(&state.ui.tab_input.typeahead_buffer) {
                    if state.ui.selectable_squares.contains(&from_square) {
                        state.select_square(from_square);
                        state.ui.tab_input.advance_to_destination(from_square);
                    } else {
                        // Invalid piece — clear buffer
                        state.ui.tab_input.typeahead_buffer.clear();
                    }
                } else {
                    state.ui.tab_input.typeahead_buffer.clear();
                }
            }
        }

        KeyCode::Enter => {
            // Only meaningful in tab 2: confirm destination
            if state.ui.tab_input.current_tab == 1 {
                let typeahead = state.ui.tab_input.typeahead_buffer.clone();
                if typeahead.len() == 2 {
                    if let Some(to_square) = parse_square(&typeahead) {
                        // Verify it's a legal destination
                        if let Some(from_square) = state.ui.tab_input.from_square {
                            if let Some(moves) = state.legal_moves_from(from_square) {
                                let to_str = chess::format_square(to_square);
                                if moves.iter().any(|m| m.to == to_str) {
                                    state.ui.tab_input.deactivate();
                                    if let Err(e) = state.try_move_to(to_square).await {
                                        state.ui.status_message =
                                            Some(format!("Move failed: {}", e));
                                    }
                                    return AppAction::Continue;
                                }
                            }
                        }
                    }
                }
                // Invalid destination — clear buffer
                state.ui.tab_input.typeahead_buffer.clear();
            }
        }

        _ => {}
    }

    AppAction::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_disabled_engine_vs_engine() {
        assert!(should_disable_input(&GameMode::EngineVsEngine));
    }

    #[test]
    fn test_input_enabled_human_vs_engine() {
        assert!(!should_disable_input(&GameMode::HumanVsEngine {
            human_side: crate::state::PlayerColor::White,
        }));
    }

    #[test]
    fn test_input_enabled_human_vs_human() {
        assert!(!should_disable_input(&GameMode::HumanVsHuman));
    }
}
