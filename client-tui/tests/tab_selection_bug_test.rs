//! Bug reproduction test - Tab selection should persist
//! This test will FAIL until the bug is fixed

use chesstty_tui::prelude::*;
use chesstty_tui::ui::fsm::UiStateMachine;
use chesstty_tui::ui::menu_app::GameConfig;
use chesstty_tui::GameMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use statig::blocking::IntoStateMachineExt;

/// This test reproduces the exact bug from the debug logs:
/// 1. Press Tab - should select first component (InfoPanel)
/// 2. Press Tab again - should navigate to next component (EnginePanel)
///
/// CURRENT BUG: Step 2 fails because selection resets to None between calls
#[test]
fn tab_press_navigates_to_next_component() {
    // Create FSM exactly as the app does
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

    // Verify initial state
    {
        let inner = unsafe { fsm.inner_mut() };
        assert_eq!(
            inner.component_manager.selected_component(),
            None,
            "Initial: nothing selected"
        );
        assert!(
            inner.component_manager.is_board_focused(),
            "Initial: Board focused"
        );
    }

    // Simulate FIRST Tab press (as input handler does)
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();
        let current = inner.component_manager.selected_component();

        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }

        // EXPECTED: InfoPanel selected
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "FIRST TAB: InfoPanel should be selected"
        );
        assert!(
            matches!(
                inner.component_manager.focus_mode,
                FocusMode::ComponentSelected {
                    component: Component::InfoPanel
                }
            ),
            "FIRST TAB: FocusMode should be ComponentSelected"
        );
    }

    // Process FSM event (this happens between key presses in real app)
    fsm.handle(&UiEvent::TimerTick);

    // Simulate SECOND Tab press
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();
        let current = inner.component_manager.selected_component();

        // THIS IS WHERE THE BUG MANIFESTS
        // With the bug: current is None, so we can't navigate
        // After fix: current should be Some(InfoPanel)
        if current.is_none() {
            panic!(
                "BUG REPRODUCED: Selection was lost! \
                Expected Some(InfoPanel), got None. \
                The ComponentManager state is not persisting between FSM events."
            );
        }

        // Navigate to next
        if let Some(curr) = current {
            if let Some(next) = inner.component_manager.next_component(curr, &layout) {
                inner.component_manager.select_component(next);
            }
        }

        // EXPECTED: Now on EnginePanel
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::EnginePanel),
            "SECOND TAB: Should navigate to EnginePanel"
        );
    }
}

/// Test that verifies selection persists through multiple FSM events
#[test]
fn selection_persists_through_fsm_events() {
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

    // Process multiple FSM events
    for i in 0..5 {
        fsm.handle(&UiEvent::TimerTick);

        let inner = unsafe { fsm.inner_mut() };
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "Selection lost after TimerTick #{} - state not persisting!",
            i
        );
    }
}

/// Test the exact scenario from debug logs
#[test]
fn debug_log_scenario_selection_persists() {
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

    // Mimic the debug log scenario exactly
    println!("Initial state check:");
    {
        let inner = unsafe { fsm.inner_mut() };
        println!("  focus_mode: {:?}", inner.component_manager.focus_mode);
        println!(
            "  selected: {:?}",
            inner.component_manager.selected_component()
        );
        assert!(inner.component_manager.is_board_focused());
    }

    // First Tab press
    println!("\nAfter first Tab:");
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();
        let current = inner.component_manager.selected_component();

        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
                println!("  Selected first: {:?}", first);
            }
        }

        println!("  focus_mode: {:?}", inner.component_manager.focus_mode);
        println!(
            "  selected: {:?}",
            inner.component_manager.selected_component()
        );
    }

    // Key event (as happens in real app)
    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };
    fsm.handle(&UiEvent::Key(key));

    // Second Tab press (THE BUG)
    println!("\nAfter second Tab:");
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();
        let current = inner.component_manager.selected_component();

        println!("  focus_mode: {:?}", inner.component_manager.focus_mode);
        println!("  selected: {:?}", current);

        // This assertion will fail if the bug exists
        assert!(
            current.is_some(),
            "BUG: selected_component() returned None after FSM handle()! \
            Expected Some(InfoPanel) from previous selection."
        );

        if let Some(curr) = current {
            if let Some(next) = inner.component_manager.next_component(curr, &layout) {
                inner.component_manager.select_component(next);
                println!("  Navigated to: {:?}", next);
            }
        }

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::EnginePanel),
            "Should be on EnginePanel after second Tab"
        );
    }
}
