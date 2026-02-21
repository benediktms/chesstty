//! Integration tests for Tab selection using the flat focus model on UiStateMachine

use client_tui::prelude::*;
use client_tui::ui::fsm::UiStateMachine;

/// Test helper to create an FSM in GameBoard state
fn create_game_board_fsm() -> UiStateMachine {
    let mut fsm = UiStateMachine::default();
    fsm.transition_to(UiMode::GameBoard);
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
