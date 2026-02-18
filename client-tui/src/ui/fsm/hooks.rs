use crate::ui::fsm::{UiEvent, UiState};

pub trait UiTransitionHook {
    fn on_before_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent);
    fn on_after_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent);
}

#[derive(Default)]
pub struct LoggingHook;

impl UiTransitionHook for LoggingHook {
    fn on_before_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {
        tracing::debug!("FSM transition: {:?} -> {:?} via {:?}", from, to, event);
    }

    fn on_after_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {
        tracing::info!("FSM transitioned: {:?} -> {:?}", from, to);
    }
}

#[derive(Default)]
pub struct RpcHook {
    pub server_address: String,
}

impl UiTransitionHook for RpcHook {
    fn on_before_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {}

    fn on_after_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {
        match (from, to) {
            (UiState::StartScreen(_), UiState::GameBoard(_)) => {
                tracing::info!("Starting new game");
            }
            (UiState::StartScreen(_), UiState::ReviewBoard(_)) => {
                tracing::info!("Starting review mode");
            }
            (UiState::GameBoard(_), UiState::MatchSummary(_)) => {
                tracing::info!("Game ended, showing summary");
            }
            _ => {}
        }
    }
}

pub struct CompositeHook<H1, H2> {
    pub hook1: H1,
    pub hook2: H2,
}

impl<H1: UiTransitionHook, H2: UiTransitionHook> UiTransitionHook for CompositeHook<H1, H2> {
    fn on_before_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {
        self.hook1.on_before_transition(from, to, event);
        self.hook2.on_before_transition(from, to, event);
    }

    fn on_after_transition(&mut self, from: &UiState, to: &UiState, event: &UiEvent) {
        self.hook1.on_after_transition(from, to, event);
        self.hook2.on_after_transition(from, to, event);
    }
}
