use crate::ui::context::FocusStack;
use crate::ui::fsm::render_spec::{Control, PaneState, RenderSpec};

#[derive(Clone, Debug, Default)]
pub struct StartScreenState {
    pub pane_state: PaneState,
    pub selected_index: usize,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
    // UI state moved from RenderState
    pub focus_stack: FocusStack,
}

impl StartScreenState {
    pub fn new() -> Self {
        let mut state = Self {
            pane_state: PaneState::start_screen(),
            selected_index: 0,
            render_spec: RenderSpec::start_screen(),
            controls: Vec::new(),
            // UI state defaults
            focus_stack: FocusStack::default(),
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        // Start screen typically shows menu items, minimal controls needed
        vec![Control::new("Enter", "Select")]
    }
}
