use crate::ui::fsm::UiMode;

pub trait UiTransitionHook {
    fn on_before_transition(&mut self, from: &UiMode, to: &UiMode);
    fn on_after_transition(&mut self, from: &UiMode, to: &UiMode);
}

#[derive(Default)]
pub struct LoggingHook;

impl UiTransitionHook for LoggingHook {
    fn on_before_transition(&mut self, from: &UiMode, to: &UiMode) {
        tracing::debug!("FSM transition: {:?} -> {:?}", from, to);
    }

    fn on_after_transition(&mut self, from: &UiMode, to: &UiMode) {
        tracing::info!("FSM transitioned: {:?} -> {:?}", from, to);
    }
}

#[derive(Default)]
pub struct RpcHook {
    pub server_address: String,
}

impl UiTransitionHook for RpcHook {
    fn on_before_transition(&mut self, _from: &UiMode, _to: &UiMode) {}

    fn on_after_transition(&mut self, from: &UiMode, to: &UiMode) {
        match (from, to) {
            (UiMode::StartScreen, UiMode::GameBoard) => {
                tracing::info!("Starting new game");
            }
            (UiMode::StartScreen, UiMode::ReviewBoard) => {
                tracing::info!("Starting review mode");
            }
            (UiMode::GameBoard, UiMode::MatchSummary) => {
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
    fn on_before_transition(&mut self, from: &UiMode, to: &UiMode) {
        self.hook1.on_before_transition(from, to);
        self.hook2.on_before_transition(from, to);
    }

    fn on_after_transition(&mut self, from: &UiMode, to: &UiMode) {
        self.hook1.on_after_transition(from, to);
        self.hook2.on_after_transition(from, to);
    }
}
