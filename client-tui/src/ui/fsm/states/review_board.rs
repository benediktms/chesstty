use crate::ui::fsm::render_spec::{
    Component, Constraint, Control, Layout, Overlay, RenderSpec, ReviewUIState, Row, Section,
    SectionContent,
};

#[derive(Clone, Debug)]
pub struct ReviewBoardState {
    pub review_tab: u8,
    pub review_moves_selection: Option<u32>,
    pub input_buffer: String,
    pub review_ui: ReviewUIState,
    pub total_plies: u32,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
}

impl Default for ReviewBoardState {
    fn default() -> Self {
        let mut state = Self {
            review_tab: 0,
            review_moves_selection: None,
            input_buffer: String::new(),
            review_ui: ReviewUIState::new(),
            total_plies: 0,
            render_spec: RenderSpec::review_board(),
            controls: Vec::new(),
        };
        state.controls = state.derive_controls();
        state
    }
}

impl ReviewBoardState {
    pub fn new(total_plies: u32) -> Self {
        let mut state = Self {
            total_plies,
            review_ui: ReviewUIState::new(),
            render_spec: RenderSpec::review_board(),
            review_tab: 0,
            review_moves_selection: None,
            input_buffer: String::new(),
            controls: Vec::new(),
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        let mut controls = Vec::new();

        // Tab navigation
        controls.push(Control::new("Tab", "Tabs"));

        // Move navigation
        controls.push(Control::new("j/k", "Moves"));

        // Critical move navigation
        controls.push(Control::new("n/p", "Critical"));

        // Auto-play toggle
        if self.review_ui.auto_play {
            controls.push(Control::new("Space", "Stop"));
        } else {
            controls.push(Control::new("Space", "Auto"));
        }

        // Jump to start/end
        controls.push(Control::new("Home/End", "Jump"));

        // Snap to critical
        controls.push(Control::new("s", "Snap"));

        // Menu
        controls.push(Control::new("Esc", "Menu"));

        controls
    }

    pub fn toggle_auto_play(&mut self) {
        self.review_ui.auto_play = !self.review_ui.auto_play;
        self.controls = self.derive_controls();
    }

    /// Derive layout based on current state
    /// Takes shared UiStateMachine for accessing component_manager
    pub fn layout(&self, _shared: &crate::ui::fsm::UiStateMachine) -> Layout {
        // Left column: Advanced Analysis (35%) on top, Review Summary below
        let left_columns = vec![
            Section::component(Constraint::Percentage(35), Component::AdvancedAnalysis),
            Section::component(Constraint::Min(10), Component::ReviewSummary),
        ];

        // Right column: Game Info on top, Move History below taking rest
        let right_columns = vec![
            Section::component(Constraint::Length(8), Component::InfoPanel),
            Section::component(Constraint::Min(10), Component::HistoryPanel),
        ];

        // Overlay is now set by UiStateMachine::layout()
        Layout {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        Section::nested(Constraint::Percentage(20), left_columns),
                        Section::component(Constraint::Percentage(55), Component::Board),
                        Section::nested(Constraint::Percentage(25), right_columns),
                    ],
                ),
                Row::new(
                    Constraint::Length(1),
                    vec![Section::component(
                        Constraint::Percentage(100),
                        Component::Controls,
                    )],
                ),
            ],
            overlay: Overlay::None, // Set by UiStateMachine
        }
    }
}
