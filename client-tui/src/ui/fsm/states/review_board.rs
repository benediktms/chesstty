use crate::ui::fsm::render_spec::{Component, Constraint, Layout, Overlay, Row, Section};

#[derive(Clone, Debug, Default)]
pub struct ReviewBoardState;

impl ReviewBoardState {
    /// Derive layout with an expanded component replacing the board
    /// Reuses the column structure from normal layout, just replacing Board with expanded pane
    pub fn layout_with_expanded(
        &self,
        component: Component,
        _shared: &crate::ui::fsm::UiStateMachine,
    ) -> Layout {
        // Left column: Advanced Analysis (35%) on top, Review Summary below
        // Dim the sidebar instance of the expanded component
        let left_columns = vec![
            Section::component(Constraint::Percentage(35), Component::AdvancedAnalysis)
                .with_dimmed(component == Component::AdvancedAnalysis),
            Section::component(Constraint::Min(10), Component::ReviewSummary)
                .with_dimmed(component == Component::ReviewSummary),
        ];

        // Right column: Game Info on top, Move History below taking rest
        let right_columns = vec![
            Section::component(Constraint::Length(8), Component::InfoPanel)
                .with_dimmed(component == Component::InfoPanel),
            Section::component(Constraint::Min(10), Component::HistoryPanel)
                .with_dimmed(component == Component::HistoryPanel),
        ];

        Layout {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        Section::nested(Constraint::Percentage(20), left_columns),
                        Section::component(Constraint::Percentage(55), component),
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
            overlay: Overlay::None,
        }
    }

    /// Derive layout based on current state
    /// Takes shared UiStateMachine for accessing visibility and focus state
    pub fn layout(&self, shared: &crate::ui::fsm::UiStateMachine) -> Layout {
        // Check for expanded component (flat focus model)
        if let Some(component) = shared.expanded_component() {
            return self.layout_with_expanded(component, shared);
        }

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
