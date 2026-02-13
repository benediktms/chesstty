use crate::app::AppState;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key_event(key: KeyEvent, app_state: &mut AppState) {
    match key.code {
        KeyCode::Char('q') => {
            // Quit handled in main loop
        }
        KeyCode::Char('u') => {
            // Undo move
            let _ = app_state.game.undo();
        }
        _ => {
            // TODO: Handle other inputs (square selection, etc.)
        }
    }
}
