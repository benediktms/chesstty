use crate::state::{ClientState, GameMode, InputPhase};
use crate::ui::context::FocusContext;
use crate::ui::pane::PaneId;
use crate::ui::widgets::popup_menu::{PopupMenuItem, PopupMenuState};
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
}

/// Returns true if character input should be disabled for the given game mode.
pub fn should_disable_input(mode: &GameMode) -> bool {
    matches!(mode, GameMode::EngineVsEngine)
}

/// Main key dispatch function. Routes input to the appropriate context handler.
pub async fn handle_key(
    state: &mut ClientState,
    input_buffer: &mut String,
    key: KeyEvent,
) -> AppAction {
    // Popup menu takes highest priority (modal overlay)
    if state.ui.popup_menu.is_some() {
        return handle_popup_input(state, key).await;
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
    match key.code {
        KeyCode::Tab => {
            // Enter pane selection mode
            if let Some(first) = state.ui.pane_manager.first_selectable() {
                state
                    .ui
                    .focus_stack
                    .push(FocusContext::PaneSelected { pane_id: first });
            }
        }
        // Pause toggle (EngineVsEngine only) — must be before Char(c) catch-all
        KeyCode::Char('p') if matches!(state.mode, GameMode::EngineVsEngine) => {
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

/// Handle keys in PaneSelected context (a pane is highlighted, user navigates/scrolls).
fn handle_pane_selected_context(
    state: &mut ClientState,
    pane_id: PaneId,
    key: KeyEvent,
) -> AppAction {
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
