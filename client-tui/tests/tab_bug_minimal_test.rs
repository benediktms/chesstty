//! Minimal reproduction of the Tab selection bug
//! This test mimics the exact flow from render_loop to catch the bug

use chesstty_tui::prelude::*;
use chesstty_tui::ui::fsm::{UiEvent, UiStateMachine};
use chesstty_tui::ui::menu_app::GameConfig;
use chesstty_tui::GameMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use statig::blocking::IntoStateMachineExt;

/// This test exactly mimics the render_loop flow:
/// 1. Create FSM
/// 2. Transition to game board
/// 3. Process TimerTick events
/// 4. Process Key events
/// 5. Check if state persists
#[test]
fn selection_lost_in_render_loop_simulation() {
    // Step 1: Create FSM (as in run_game line 213)
    let mut fsm = UiStateMachine::default().state_machine();

    // Step 2: Transition to game board (as in run_game line 357)
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

    // Step 3: Simulate some TimerTick events (as in render_loop line 427)
    for _ in 0..3 {
        fsm.handle(&UiEvent::TimerTick);
    }

    // Step 4: Process first Tab key (as in render_loop lines 488-490)
    let tab_key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };

    // First, fsm.handle is called
    fsm.handle(&UiEvent::Key(tab_key));

    // Then input::handle_key logic (simplified)
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = Layout::game_board();
        let current = inner.component_manager.selected_component();

        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }

        // Verify first Tab worked
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "First Tab should select InfoPanel"
        );
    }

    // Step 5: Simulate more TimerTick events (time passing between key presses)
    for _ in 0..5 {
        fsm.handle(&UiEvent::TimerTick);
    }

    // Step 6: Process second Tab key
    fsm.handle(&UiEvent::Key(tab_key));

    // Check if state persisted
    {
        let inner = unsafe { fsm.inner_mut() };
        let current = inner.component_manager.selected_component();

        // THE BUG: current is None here!
        if current.is_none() {
            panic!(
                "BUG REPRODUCED: Selection was lost between Tab presses!\n\
                This simulates the exact render_loop flow.\n\
                The state is being reset somewhere in the FSM/StateMachine."
            );
        }

        // If we get here, the bug is fixed
        assert_eq!(
            current,
            Some(Component::InfoPanel),
            "Selection should have persisted"
        );
    }
}

/// Test with just the minimal operations
#[test]
fn minimal_reproduction() {
    let mut fsm = UiStateMachine::default().state_machine();

    // Setup
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
            Some(Component::InfoPanel)
        );
        // Log the memory address to track if ComponentManager is being replaced
        tracing::debug!("ComponentManager address: {:p}", &inner.component_manager);
    }

    // Process events that happen between key presses
    fsm.handle(&UiEvent::TimerTick);
    fsm.handle(&UiEvent::TimerTick);

    // Check if it persisted
    {
        let inner = unsafe { fsm.inner_mut() };
        tracing::debug!("ComponentManager address: {:p}", &inner.component_manager);

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "Selection was lost - this is the bug!"
        );
    }
}
