use crate::ui::fsm::component::Component;
use crate::ui::fsm::render_spec::{Control, Layout, RenderSpec};

#[derive(Clone, Debug)]
pub struct ReviewBoardPaneFocusedState {
    pub focused_component: Component,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
}

impl Default for ReviewBoardPaneFocusedState {
    fn default() -> Self {
        let mut state = Self {
            focused_component: Component::ReviewSummary,
            render_spec: RenderSpec::review_board(),
            controls: Vec::new(),
        };
        state.controls = state.derive_controls();
        state
    }
}

impl ReviewBoardPaneFocusedState {
    pub fn new(component: Component) -> Self {
        let mut state = Self {
            focused_component: component,
            render_spec: RenderSpec::review_board_with_pane(component),
            controls: Vec::new(),
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        let mut controls = Vec::new();

        controls.push(Control::new("↑↓", "Scroll"));
        controls.push(Control::new("PgUp/PgDn", "Top/Bottom"));

        controls.push(Control::new("Esc", "Back"));

        controls
    }

    pub fn layout(&self, _shared: &crate::ui::fsm::UiStateMachine) -> Layout {
        self.render_spec.layout.clone()
    }
}
