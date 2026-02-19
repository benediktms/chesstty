//! Integration tests for Tab selection persistence
//! These tests verify that component selection survives FSM state transitions

use chesstty_tui::menu_app::GameConfig;
use chesstty_tui::prelude::*;
use chesstty_tui::ui::fsm::UiStateMachine;

/// Test helper to create an FSM in GameBoard state
fn create_game_board_fsm() -> UiStateMachine {
    let mut fsm = UiStateMachine::default();

    // Transition to game board
    // Note: We manipulate the FSM directly without the StateMachine wrapper
    // to avoid the borrow checker issues with statig's StateMachine
    fsm.current_state = chesstty_tui::prelude::UiState::GameBoard(
        chesstty_tui::prelude::GameBoardState::new(GameMode::HumanVsHuman),
    );
    fsm.setup_game_mode();

    fsm
}

#[test]
fn initial_state_is_board_focused() {
    let fsm = create_game_board_fsm();

    // Initial state: Board focused, nothing selected
    assert!(fsm.component_manager.is_board_focused());
    assert_eq!(fsm.component_manager.selected_component(), None);
}

#[test]
fn selecting_component_changes_focus_mode() {
    let mut fsm = create_game_board_fsm();

    // Select a component directly
    fsm.component_manager.select_component(Component::InfoPanel);

    // Focus mode should change
    assert!(!fsm.component_manager.is_board_focused());
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::InfoPanel)
    );
}

#[test]
fn selection_persists_after_multiple_selections() {
    let mut fsm = create_game_board_fsm();
    let layout = Layout::game_board();

    // Select first component
    fsm.component_manager.select_component(Component::InfoPanel);
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::InfoPanel)
    );

    // Select next component
    let next = fsm
        .component_manager
        .next_component(Component::InfoPanel, &layout);
    assert_eq!(next, Some(Component::EnginePanel));

    fsm.component_manager.select_component(next.unwrap());
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::EnginePanel)
    );

    // Select next component
    let next = fsm
        .component_manager
        .next_component(Component::EnginePanel, &layout);
    assert_eq!(next, Some(Component::HistoryPanel));

    fsm.component_manager.select_component(next.unwrap());
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::HistoryPanel)
    );

    // Verify focus mode tracks correctly
    assert!(
        matches!(
            fsm.component_manager.focus_mode,
            FocusMode::ComponentSelected {
                component: Component::HistoryPanel
            }
        ),
        "Focus mode should track the currently selected component"
    );
}

#[test]
fn tab_navigation_works_correctly() {
    let mut fsm = create_game_board_fsm();
    let layout = Layout::game_board();

    // Initial state: nothing selected
    assert_eq!(fsm.component_manager.selected_component(), None);

    // Get first component and select it (simulating Tab press)
    let first = fsm.component_manager.first_component(&layout);
    assert_eq!(first, Some(Component::InfoPanel));

    fsm.component_manager.select_component(first.unwrap());
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::InfoPanel)
    );

    // Get next component (simulating second Tab press)
    let next = fsm
        .component_manager
        .next_component(Component::InfoPanel, &layout);
    assert_eq!(next, Some(Component::EnginePanel));

    fsm.component_manager.select_component(next.unwrap());
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::EnginePanel),
        "Should navigate to EnginePanel"
    );

    // Verify focus mode is still ComponentSelected
    assert!(
        matches!(
            fsm.component_manager.focus_mode,
            FocusMode::ComponentSelected {
                component: Component::EnginePanel
            }
        ),
        "Focus mode should track the selected component"
    );
}

#[test]
fn full_tab_workflow() {
    let mut fsm = create_game_board_fsm();
    let layout = Layout::game_board();

    // Initial state
    assert_eq!(fsm.component_manager.selected_component(), None);
    assert!(fsm.component_manager.is_board_focused());

    // First Tab press
    let first = fsm.component_manager.first_component(&layout).unwrap();
    fsm.component_manager.select_component(first);

    // Selection should persist
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::InfoPanel),
        "Selection should persist"
    );

    // Navigate to next
    let next = fsm
        .component_manager
        .next_component(Component::InfoPanel, &layout)
        .unwrap();
    fsm.component_manager.select_component(next);

    // Should still be on EnginePanel
    assert_eq!(
        fsm.component_manager.selected_component(),
        Some(Component::EnginePanel),
        "Should remain on EnginePanel"
    );
}

#[test]
fn focus_mode_can_be_cleared() {
    let mut fsm = create_game_board_fsm();

    // Select a component
    fsm.component_manager.select_component(Component::InfoPanel);
    assert!(!fsm.component_manager.is_board_focused());

    // Clear focus
    fsm.component_manager.clear_focus();

    // Should be back to Board focus
    assert!(fsm.component_manager.is_board_focused());
    assert_eq!(fsm.component_manager.selected_component(), None);
}

#[test]
fn expanding_component_changes_focus_mode() {
    let mut fsm = create_game_board_fsm();

    // Expand a component
    fsm.component_manager
        .expand_component(Component::HistoryPanel);

    assert!(
        matches!(
            fsm.component_manager.focus_mode,
            FocusMode::ComponentExpanded {
                component: Component::HistoryPanel
            }
        ),
        "Focus mode should be ComponentExpanded"
    );
    assert_eq!(
        fsm.component_manager.expanded_component(),
        Some(Component::HistoryPanel)
    );
}

/// This test documents the actual bug - selection doesn't persist through FSM handle() calls
/// When using the statig StateMachine wrapper, the state appears to be cloned/reset
#[test]
fn selection_persists_through_fsm_state_machine() {
    use crossterm::event::{KeyCode, KeyEvent};
    use statig::blocking::IntoStateMachineExt;

    // Create the FSM wrapped in StateMachine (as the actual app does)
    let mut fsm = UiStateMachine::default().state_machine();

    // Transition to game board
    fsm.handle(&UiEvent::StartGame(GameConfig {
        mode: GameMode::HumanVsHuman,
        skill_level: 0,
        start_fen: None,
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
        pre_history: None,
        advanced_data: None,
    }));

    // Access the inner FSM to set up state
    // Note: We need unsafe here because statig's StateMachine doesn't provide safe mutable access
    let inner = unsafe { fsm.inner_mut() };

    // Select a component
    inner
        .component_manager
        .select_component(Component::InfoPanel);
    assert_eq!(
        inner.component_manager.selected_component(),
        Some(Component::InfoPanel),
        "Should be selected before handle() call"
    );

    // Process a key event through the StateMachine
    let key = KeyEvent::from(KeyCode::Char('a'));
    fsm.handle(&UiEvent::Key(key));

    // Access inner again to check state
    let inner = unsafe { fsm.inner_mut() };

    // Verify selection persists
    assert_eq!(
        inner.component_manager.selected_component(),
        Some(Component::InfoPanel),
        "Selection should persist through FSM handle() call"
    );
}
