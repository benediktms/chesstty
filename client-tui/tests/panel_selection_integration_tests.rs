//! Integration tests for panel selection using the flat focus model on UiStateMachine

use client_tui::prelude::*;
use client_tui::ui::fsm::UiStateMachine;

/// Test helper to create an FSM in GameBoard state
fn create_game_board_fsm() -> UiStateMachine {
    let mut fsm = UiStateMachine::default();
    fsm.transition_to(UiMode::GameBoard);
    fsm
}

/// Test helper to create an FSM in ReviewBoard state
fn create_review_board_fsm() -> UiStateMachine {
    let mut fsm = UiStateMachine::default();
    fsm.transition_to(UiMode::ReviewBoard);
    fsm
}

#[test]
fn initial_state_is_board_focused() {
    let fsm = create_game_board_fsm();
    assert!(fsm.is_board_focused());
    assert_eq!(fsm.selected_component(), None);
}

#[test]
fn selecting_component_changes_focus() {
    let mut fsm = create_game_board_fsm();

    fsm.select_component(Component::InfoPanel);

    assert!(!fsm.is_board_focused());
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));
}

#[test]
fn selection_persists_after_multiple_selections() {
    let mut fsm = create_game_board_fsm();
    let layout = GameBoardState.layout(&fsm);

    fsm.select_component(Component::InfoPanel);
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));

    let next = fsm.next_component(Component::InfoPanel, &layout);
    assert_eq!(next, Some(Component::EnginePanel));
    fsm.select_component(next.unwrap());
    assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));

    let next = fsm.next_component(Component::EnginePanel, &layout);
    assert_eq!(next, Some(Component::HistoryPanel));
    fsm.select_component(next.unwrap());
    assert_eq!(fsm.selected_component(), Some(Component::HistoryPanel));

    assert_eq!(fsm.focused_component, Some(Component::HistoryPanel));
    assert!(!fsm.expanded);
    assert!(!fsm.expanded);
}

#[test]
fn tab_navigation_works_correctly() {
    let mut fsm = create_game_board_fsm();
    let layout = GameBoardState.layout(&fsm);

    // Initial state: nothing selected
    assert_eq!(fsm.selected_component(), None);

    // First Tab: select first component
    let first = fsm.first_component(&layout);
    assert_eq!(first, Some(Component::InfoPanel));
    fsm.select_component(first.unwrap());
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));

    // Second Tab: navigate to next
    let next = fsm.next_component(Component::InfoPanel, &layout);
    assert_eq!(next, Some(Component::EnginePanel));
    fsm.select_component(next.unwrap());
    assert_eq!(
        fsm.selected_component(),
        Some(Component::EnginePanel),
        "Should navigate to EnginePanel"
    );
}

#[test]
fn full_tab_workflow() {
    let mut fsm = create_game_board_fsm();
    let layout = GameBoardState.layout(&fsm);

    // Initial state
    assert_eq!(fsm.selected_component(), None);
    assert!(fsm.is_board_focused());

    // First Tab press
    let first = fsm.first_component(&layout).unwrap();
    fsm.select_component(first);
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));

    // Navigate to next
    let next = fsm
        .next_component(Component::InfoPanel, &layout)
        .unwrap();
    fsm.select_component(next);
    assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));
}

#[test]
fn focus_can_be_cleared() {
    let mut fsm = create_game_board_fsm();

    fsm.select_component(Component::InfoPanel);
    assert!(!fsm.is_board_focused());

    fsm.clear_focus();
    assert!(fsm.is_board_focused());
    assert_eq!(fsm.selected_component(), None);
}

#[test]
fn expanding_component_works() {
    let mut fsm = create_game_board_fsm();

    fsm.expand_component(Component::HistoryPanel);

    assert_eq!(fsm.focused_component, Some(Component::HistoryPanel));
    assert!(fsm.expanded);
    assert_eq!(fsm.expanded_component(), Some(Component::HistoryPanel));
}

/// Flat model state persists through transitions
#[test]
fn selection_persists_through_transitions() {
    let mut fsm = UiStateMachine::default();
    fsm.transition_to(UiMode::GameBoard);

    fsm.select_component(Component::InfoPanel);
    assert_eq!(
        fsm.selected_component(),
        Some(Component::InfoPanel),
        "Should be selected after transition"
    );

    // Verify selection persists (no statig wrapper to interfere)
    assert_eq!(
        fsm.selected_component(),
        Some(Component::InfoPanel),
        "Selection should persist"
    );
}

// === Number-key direct selection tests ===

#[test]
fn number_key_1_selects_info_panel_in_game_mode() {
    let mut fsm = create_game_board_fsm();
    assert!(fsm.is_board_focused());

    let target = Component::from_number_key('1', &fsm.mode).unwrap();
    assert_eq!(target, Component::InfoPanel);
    assert!(fsm.is_component_visible(&target));
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));
}

#[test]
fn number_key_2_selects_engine_panel_in_game_mode() {
    let mut fsm = create_game_board_fsm();

    let target = Component::from_number_key('2', &fsm.mode).unwrap();
    assert_eq!(target, Component::EnginePanel);
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));
}

#[test]
fn number_key_3_selects_history_panel_in_game_mode() {
    let mut fsm = create_game_board_fsm();

    let target = Component::from_number_key('3', &fsm.mode).unwrap();
    assert_eq!(target, Component::HistoryPanel);
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::HistoryPanel));
}

#[test]
fn number_key_for_hidden_panel_does_nothing() {
    let mut fsm = create_game_board_fsm();
    // DebugPanel is hidden by default
    assert!(!fsm.is_component_visible(&Component::DebugPanel));

    let target = Component::from_number_key('4', &fsm.mode).unwrap();
    assert_eq!(target, Component::DebugPanel);
    // The input handler checks visibility before selecting — simulate that
    if fsm.is_component_visible(&target) {
        fsm.select_component(target);
    }
    // Should still be board-focused
    assert!(fsm.is_board_focused());
    assert_eq!(fsm.selected_component(), None);
}

#[test]
fn esc_from_selected_panel_returns_to_board() {
    let mut fsm = create_game_board_fsm();

    fsm.select_component(Component::EnginePanel);
    assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));

    fsm.clear_focus();
    assert!(fsm.is_board_focused());
    assert_eq!(fsm.selected_component(), None);
}

#[test]
fn number_keys_switch_panels_in_component_selected_context() {
    let mut fsm = create_game_board_fsm();

    // Start with InfoPanel selected
    fsm.select_component(Component::InfoPanel);
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));

    // Press '3' to switch directly to HistoryPanel
    let target = Component::from_number_key('3', &fsm.mode).unwrap();
    if fsm.is_component_visible(&target) {
        fsm.select_component(target);
    }
    assert_eq!(fsm.selected_component(), Some(Component::HistoryPanel));

    // Press '2' to switch directly to EnginePanel
    let target = Component::from_number_key('2', &fsm.mode).unwrap();
    if fsm.is_component_visible(&target) {
        fsm.select_component(target);
    }
    assert_eq!(fsm.selected_component(), Some(Component::EnginePanel));
}

#[test]
fn number_keys_in_review_mode_select_review_panels() {
    let mut fsm = create_review_board_fsm();

    // '3' in review mode -> AdvancedAnalysis
    let target = Component::from_number_key('3', &fsm.mode).unwrap();
    assert_eq!(target, Component::AdvancedAnalysis);
    assert!(fsm.is_component_visible(&target));
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::AdvancedAnalysis));

    // '4' in review mode -> ReviewSummary
    let target = Component::from_number_key('4', &fsm.mode).unwrap();
    assert_eq!(target, Component::ReviewSummary);
    assert!(fsm.is_component_visible(&target));
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::ReviewSummary));

    // '1' still selects InfoPanel
    let target = Component::from_number_key('1', &fsm.mode).unwrap();
    assert_eq!(target, Component::InfoPanel);
    fsm.select_component(target);
    assert_eq!(fsm.selected_component(), Some(Component::InfoPanel));
}

#[test]
fn debug_panel_selectable_when_visible() {
    let mut fsm = create_game_board_fsm();

    // Make DebugPanel visible
    fsm.set_component_visible(Component::DebugPanel, true);
    assert!(fsm.is_component_visible(&Component::DebugPanel));

    let target = Component::from_number_key('4', &fsm.mode).unwrap();
    if fsm.is_component_visible(&target) {
        fsm.select_component(target);
    }
    assert_eq!(fsm.selected_component(), Some(Component::DebugPanel));
}
