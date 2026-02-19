//! Bug reproduction test - Tab selection persistence
//! This test mimics the EXACT flow from the running application

use chesstty_tui::prelude::*;
use chesstty_tui::ui::fsm::{UiEvent, UiStateMachine};
use chesstty_tui::ui::menu_app::GameConfig;
use chesstty_tui::GameMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use statig::blocking::IntoStateMachineExt;

/// Test without needing GameSession - directly test FSM behavior
/// This test modifies ComponentManager and verifies changes persist through fsm.handle()
#[test]
fn component_manager_survives_fsm_handle_calls() {
    let mut fsm = UiStateMachine::default().state_machine();

    // Setup - transition to game board
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

    // Modify state directly (as input::handle_key does)
    {
        let inner = unsafe { fsm.inner_mut() };
        inner
            .component_manager
            .select_component(Component::HistoryPanel);
        inner
            .component_manager
            .set_visible(Component::EnginePanel, false);

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::HistoryPanel),
            "Should be selected after explicit selection"
        );
        assert!(
            !inner.component_manager.is_visible(&Component::EnginePanel),
            "Visibility should be modified"
        );
        assert!(
            matches!(
                inner.component_manager.focus_mode,
                FocusMode::ComponentSelected {
                    component: Component::HistoryPanel
                }
            ),
            "Focus mode should be updated"
        );
    }

    // Process FSM event (as happens in render_loop between key presses)
    let key = KeyEvent::from(KeyCode::Char('a'));
    fsm.handle(&UiEvent::Key(key));

    // Verify ALL state survived - THIS TEST WILL FAIL if there's a bug
    {
        let inner = unsafe { fsm.inner_mut() };

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::HistoryPanel),
            "CRITICAL BUG: selected_component was reset by fsm.handle()!"
        );

        assert!(
            !inner.component_manager.is_visible(&Component::EnginePanel),
            "CRITICAL BUG: visibility was reset by fsm.handle()!"
        );

        assert!(
            matches!(
                inner.component_manager.focus_mode,
                FocusMode::ComponentSelected {
                    component: Component::HistoryPanel
                }
            ),
            "CRITICAL BUG: focus_mode was reset by fsm.handle()!"
        );
    }
}

/// Test the exact Tab navigation scenario
#[test]
fn tab_navigation_state_persists() {
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

    let layout = Layout::game_board();

    // FIRST TAB
    {
        let inner = unsafe { fsm.inner_mut() };
        let current = inner.component_manager.selected_component();

        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "First Tab should select InfoPanel"
        );
    }

    // FSM event between key presses
    fsm.handle(&UiEvent::TimerTick);

    // SECOND TAB
    {
        let inner = unsafe { fsm.inner_mut() };
        let current = inner.component_manager.selected_component();

        // This is the bug check - current should NOT be None
        if current.is_none() {
            panic!(
                "BUG: Selection was lost! Expected Some(InfoPanel), got None. \
                The component_manager state was reset by fsm.handle()"
            );
        }

        // Navigate to next
        if let Some(curr) = current {
            if let Some(next) = inner.component_manager.next_component(curr, &layout) {
                inner.component_manager.select_component(next);
            }
        }

        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::EnginePanel),
            "Second Tab should navigate to EnginePanel"
        );
    }
}
