//! Bug reproduction test - asserts the CURRENT WRONG behavior
//! This test documents what IS happening (not what SHOULD happen)
//!
//! CURRENT BEHAVIOR (WRONG):
//! - First Tab: selected_component() returns None -> select InfoPanel
//! - Second Tab: selected_component() returns None -> should return InfoPanel!
//!
//! EXPECTED BEHAVIOR (CORRECT):
//! - First Tab: selected_component() returns None -> select InfoPanel  
//! - Second Tab: selected_component() returns Some(InfoPanel) -> navigate to EnginePanel

use chesstty_tui::prelude::*;
use chesstty_tui::ui::fsm::{UiEvent, UiStateMachine};
use chesstty_tui::ui::menu_app::GameConfig;
use chesstty_tui::GameMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use statig::blocking::IntoStateMachineExt;

/// This test documents the EXACT bug from the debug logs:
///
/// Log output showing the bug:
/// ```
/// Tab pressed - focus_mode: Board
/// Current selected: None          <-- WRONG: Should be InfoPanel after first Tab!
/// First component: Some(InfoPanel)
/// Selected first: InfoPanel
/// ```
#[test]
fn tab_press_current_selected_should_be_infopanel() {
    let mut fsm = UiStateMachine::default().state_machine();

    // Setup game board
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

    // FIRST TAB PRESS
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();

        // Check initial state
        assert_eq!(
            inner.component_manager.selected_component(),
            None,
            "Before first Tab: nothing selected (correct)"
        );

        // Simulate Tab press logic
        let current = inner.component_manager.selected_component();
        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }

        // Verify selection worked
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "After first Tab: InfoPanel selected"
        );
    }

    // SECOND TAB PRESS (this is where the bug manifests)
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();

        // Get current selection
        let current = inner.component_manager.selected_component();

        // THIS IS THE BUG!
        // current is None, but it should be Some(InfoPanel)
        // The test below documents the WRONG behavior
        assert!(
            current.is_none(),
            "BUG: current is None but should be Some(InfoPanel). \
            This assertion will FAIL once the bug is fixed, \
            which is the correct behavior."
        );

        // Because current is None, we can't navigate to next
        // We end up selecting InfoPanel again instead of EnginePanel
        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }

        // We're stuck on InfoPanel instead of navigating to EnginePanel
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "WRONG: Still on InfoPanel. Should be EnginePanel after second Tab."
        );
    }
}

/// This test will PASS when the bug exists and FAIL when fixed
/// It's the inverse of what we want long-term
#[test]
fn selection_does_not_persist_between_tabs() {
    let mut fsm = UiStateMachine::default().state_machine();

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

    // Select a component
    {
        let inner = unsafe { fsm.inner_mut() };
        inner
            .component_manager
            .select_component(Component::InfoPanel);
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "Should be selected"
        );
    }

    // Simulate time passing (as happens in real app between key presses)
    // In real app: TimerTick events happen every 33ms
    for _ in 0..10 {
        fsm.handle(&UiEvent::TimerTick);
    }

    // Check if selection persisted
    {
        let inner = unsafe { fsm.inner_mut() };
        let current = inner.component_manager.selected_component();

        // THIS ASSERTION DOCUMENTS THE BUG
        // It passes when the bug exists (current is None)
        // It will FAIL when we fix the bug (current will be Some(InfoPanel))
        assert!(
            current.is_none(),
            "This assertion documents the bug. \
            It passes when selection is lost (bug exists). \
            It will FAIL when selection persists (bug fixed). \
            Current value: {:?}",
            current
        );
    }
}
