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
    if state.ui_state.popup_menu.is_some() {
        return handle_popup_input(state, key).await;
    }

    // Promotion dialog takes priority (modal overlay)
    if matches!(state.ui_state.input_phase, InputPhase::SelectPromotion { .. }) {
        return handle_promotion_input(state, input_buffer, key);
    }

    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return AppAction::Quit;
    }

    // Global toggles that work in any context
    match key.code {
        KeyCode::Char('@') => {
            state.ui_state.pane_manager.toggle_visibility(PaneId::UciDebug);
            return AppAction::Continue;
        }
        KeyCode::Char('#') => {
            state.ui_state.pane_manager.toggle_visibility(PaneId::EngineAnalysis);
            return AppAction::Continue;
        }
        _ => {}
    }

    // Dispatch by context
    let context = state.ui_state.focus_stack.current().clone();
    match context {
        FocusContext::Board => {
            handle_board_context(state, input_buffer, key).await
        }
        FocusContext::PaneSelected { pane_id } => {
            handle_pane_selected_context(state, pane_id, key)
        }
        FocusContext::PaneExpanded { pane_id } => {
            handle_pane_expanded_context(state, pane_id, key)
        }
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
            if let Some(first) = state.ui_state.pane_manager.first_selectable() {
                state.ui_state.focus_stack.push(FocusContext::PaneSelected {
                    pane_id: first,
                });
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
            // If there's a selection, clear it first. Otherwise show popup menu.
            if state.ui_state.selected_square.is_some() {
                state.clear_selection();
                input_buffer.clear();
            } else {
                input_buffer.clear();
                state.ui_state.popup_menu = Some(PopupMenuState::new(&state.mode));
            }
        }
        _ => {}
    }
    AppAction::Continue
}

/// Handle keys when the popup menu is active.
async fn handle_popup_input(
    state: &mut ClientState,
    key: KeyEvent,
) -> AppAction {
    match key.code {
        KeyCode::Up => {
            if let Some(ref mut menu) = state.ui_state.popup_menu {
                menu.move_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut menu) = state.ui_state.popup_menu {
                menu.move_down();
            }
        }
        KeyCode::Enter => {
            let selected = state
                .ui_state
                .popup_menu
                .as_ref()
                .map(|m| m.selected_item().clone());

            state.ui_state.popup_menu = None; // Close menu

            if let Some(item) = selected {
                match item {
                    PopupMenuItem::Restart => {
                        if let Err(e) = state.reset(None).await {
                            state.ui_state.status_message =
                                Some(format!("Reset error: {}", e));
                        }
                    }
                    PopupMenuItem::AdjustDifficulty => {
                        // Cycle through difficulty levels: 3 -> 10 -> 15 -> 20 -> 3
                        let new_level = match state.skill_level {
                            0..=5 => 10,
                            6..=12 => 15,
                            13..=18 => 20,
                            _ => 3,
                        };
                        if let Err(e) = state.set_engine(true, new_level).await {
                            state.ui_state.status_message =
                                Some(format!("Engine error: {}", e));
                        } else {
                            let label = match new_level {
                                3 => "Beginner",
                                10 => "Intermediate",
                                15 => "Advanced",
                                20 => "Master",
                                _ => "Custom",
                            };
                            state.ui_state.status_message =
                                Some(format!("Difficulty set to {}", label));
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
            state.ui_state.popup_menu = None; // Dismiss menu
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
        KeyCode::Left => {
            if let Some(prev) = state.ui_state.pane_manager.prev_selectable(pane_id) {
                state.ui_state.focus_stack.pop();
                state.ui_state.focus_stack.push(FocusContext::PaneSelected {
                    pane_id: prev,
                });
            }
        }
        KeyCode::Right | KeyCode::Tab => {
            if let Some(next) = state.ui_state.pane_manager.next_selectable(pane_id) {
                state.ui_state.focus_stack.pop();
                state.ui_state.focus_stack.push(FocusContext::PaneSelected {
                    pane_id: next,
                });
            }
        }
        KeyCode::Up => {
            let scroll = state.ui_state.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_sub(5);
        }
        KeyCode::Down => {
            let scroll = state.ui_state.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_add(5);
        }
        KeyCode::PageUp => {
            *state.ui_state.pane_manager.scroll_mut(pane_id) = 0;
        }
        KeyCode::PageDown => {
            *state.ui_state.pane_manager.scroll_mut(pane_id) = u16::MAX;
        }
        KeyCode::Enter => {
            use crate::ui::pane::pane_properties;
            let props = pane_properties(pane_id);
            if props.is_expandable {
                state.ui_state.focus_stack.push(FocusContext::PaneExpanded {
                    pane_id,
                });
            }
        }
        KeyCode::Esc => {
            state.ui_state.focus_stack.pop();
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
        KeyCode::Up => {
            let scroll = state.ui_state.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_sub(5);
        }
        KeyCode::Down => {
            let scroll = state.ui_state.pane_manager.scroll_mut(pane_id);
            *scroll = scroll.saturating_add(5);
        }
        KeyCode::PageUp => {
            *state.ui_state.pane_manager.scroll_mut(pane_id) = 0;
        }
        KeyCode::PageDown => {
            *state.ui_state.pane_manager.scroll_mut(pane_id) = u16::MAX;
        }
        KeyCode::Esc => {
            state.ui_state.focus_stack.pop();
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
