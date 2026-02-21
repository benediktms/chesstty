use crate::ui::fsm::render_spec::{Component, Constraint, Layout, Overlay, Row, Section};

#[derive(Clone, Debug, Default)]
pub struct GameBoardState;

impl GameBoardState {
    /// Derive layout with an expanded component replacing the board
    /// Reuses the column structure from normal layout, just replacing Board with expanded pane
    pub fn layout_with_expanded(
        &self,
        component: Component,
        shared: &crate::ui::fsm::UiStateMachine,
    ) -> Layout {
        // Build center column - expanded pane with optional tab input
        let mut center_columns = vec![Section::component(Constraint::Min(10), component)];

        // Only include TabInput if active (from shared state)
        if shared.tab_input.active {
            center_columns.push(Section::component(
                Constraint::Length(3),
                Component::TabInput,
            ));
        }

        // Build right column - stacked: GameInfo → EngineAnalysis → MoveHistory
        let mut right_columns = vec![Section::component(
            Constraint::Length(8),
            Component::InfoPanel,
        )];

        // Only include EnginePanel if visible (from shared state)
        if shared.is_component_visible(&Component::EnginePanel) {
            right_columns.push(Section::component(
                Constraint::Length(12),
                Component::EnginePanel,
            ));
        }

        // Move history takes remaining space
        right_columns.push(Section::component(
            Constraint::Min(10),
            Component::HistoryPanel,
        ));

        Layout {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left column empty (space redistributed to center/right)
                        Section::nested(Constraint::Percentage(75), center_columns),
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

    /// Derive layout based on current state
    /// Takes shared UiStateMachine for accessing pane_manager, tab_input, etc.
    pub fn layout(&self, shared: &crate::ui::fsm::UiStateMachine) -> Layout {
        // Check for expanded component (flat focus model)
        if let Some(component) = shared.expanded_component() {
            return self.layout_with_expanded(component, shared);
        }

        // Build center column - board with optional tab input
        let mut center_columns = vec![Section::component(Constraint::Min(10), Component::Board)];

        // Only include TabInput if active (from shared state)
        if shared.tab_input.active {
            center_columns.push(Section::component(
                Constraint::Length(3),
                Component::TabInput,
            ));
        }

        // Build right column - stacked: GameInfo → EngineAnalysis → MoveHistory
        let mut right_columns = vec![Section::component(
            Constraint::Length(8),
            Component::InfoPanel,
        )];

        // Only include EnginePanel if visible (from shared state)
        if shared.is_component_visible(&Component::EnginePanel) {
            right_columns.push(Section::component(
                Constraint::Length(12),
                Component::EnginePanel,
            ));
        }

        // Move history takes remaining space
        right_columns.push(Section::component(
            Constraint::Min(10),
            Component::HistoryPanel,
        ));

        // Overlay is now set by UiStateMachine::layout()
        Layout {
            rows: vec![
                Row::new(
                    Constraint::Percentage(95),
                    vec![
                        // Left column empty (space redistributed to center/right)
                        Section::nested(Constraint::Percentage(75), center_columns),
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
