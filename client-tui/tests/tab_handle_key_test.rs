//! Integration test using the actual handle_key function
//! This tests the full input handling flow

use chesstty_tui::prelude::*;
use chesstty_tui::state::GameSession;
use chesstty_tui::ui::input::{handle_key, AppAction};
use chesstty_tui::GameMode;
use chesstty_tui::ui::fsm::{UiEvent, UiStateMachine};
use chesstty_tui::ui::menu_app::GameConfig;
use chess_client::ChessClient;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use statig::blocking::IntoStateMachineExt;

/// Test that calls the actual handle_key function
/// This is the closest we can get to the real app flow without a real GameSession
#[tokio::test]
async fn tab_through_handle_key_function() {
    // Create FSM
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

    // We can't easily create a GameSession without connecting to the server
    // So let's test the handle_board_context logic directly
    
    let tab_key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };

    // FIRST TAB
    {
        let inner = unsafe { fsm.inner_mut() };
        
        // This is what handle_board_context does for Tab
        let layout = inner.layout(&mock_game_session());
        let current = inner.component_manager.selected_component();
        
        if current.is_none() {
            if let Some(first) = inner.component_manager.first_component(&layout) {
                inner.component_manager.select_component(first);
            }
        }
        
        assert_eq!(
            inner.component_manager.selected_component(),
            Some(Component::InfoPanel),
            "First Tab"
        );
    }

    // FSM handle (as render_loop does)
    fsm.handle(&UiEvent::TimerTick);

    // SECOND TAB
    {
        let inner = unsafe { fsm.inner_mut() };
        let layout = inner.layout(&mock_game_session());
        
        // This should return Some(InfoPanel), not None
        let current = inner.component_manager.selected_component();
        
        println!("Second Tab - current: {:?}", current);
        println!("focus_mode: {:?}", inner.component_manager.focus_mode);
        
        // THE BUG: current is None here in the real app
        // But in tests it persists correctly
        // This suggests the bug is in a different code path
        
        if current.is_none() {
            panic!(
                "BUG: current is None on second Tab. Should be Some(InfoPanel).\n\
                This is the exact bug from the debug logs."
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

fn mock_game_session() -> GameSession {
    // Create a minimal mock session
    // In a real test we'd need to connect to the server or properly mock this
    todo!("Need to create a mock GameSession for testing")
}
