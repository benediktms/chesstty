use crate::ui::fsm::render_spec::{Control, RenderSpec};

#[derive(Clone, Debug, Default)]
pub struct StartScreenState {
    pub selected_index: usize,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
}

impl StartScreenState {
    pub fn new() -> Self {
        let mut state = Self {
            selected_index: 0,
            render_spec: RenderSpec::start_screen(),
            controls: Vec::new(),
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        vec![Control::new("Enter", "Select")]
    }
}
