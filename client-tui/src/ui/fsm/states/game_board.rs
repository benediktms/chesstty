use crate::state::GameMode;
use crate::ui::fsm::render_spec::{
    Component, Constraint, Control, InputPhase, Layout, Overlay, RenderSpec, Row, Section,
    SectionContent, TabInputState,
};
use crate::ui::widgets::popup_menu::PopupMenuState;
use crate::ui::widgets::snapshot_dialog::SnapshotDialogState;

#[derive(Clone, Debug)]
pub struct GameBoardState {
    pub tab_input: TabInputState,
    pub input_phase: InputPhase,
    pub input_buffer: String,
    pub game_mode: GameMode,
    pub move_count: u32,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
    pub popup_menu: Option<PopupMenuState>,
    pub snapshot_dialog: Option<SnapshotDialogState>,
    pub paused: bool,
}

impl Default for GameBoardState {
    fn default() -> Self {
        let mut state = Self {
            tab_input: TabInputState::new(),
            input_phase: InputPhase::SelectPiece,
            input_buffer: String::new(),
            game_mode: GameMode::HumanVsHuman,
            move_count: 0,
            render_spec: RenderSpec::game_board(),
            controls: Vec::new(),
            popup_menu: None,
            snapshot_dialog: None,
            paused: false,
        };
        state.controls = state.derive_controls();
        state
    }
}

impl GameBoardState {
    pub fn new(game_mode: GameMode) -> Self {
        let mut state = Self {
            game_mode,
            tab_input: TabInputState::new(),
            input_phase: InputPhase::SelectPiece,
            render_spec: RenderSpec::game_board(),
            input_buffer: String::new(),
            move_count: 0,
            controls: Vec::new(),
            popup_menu: None,
            snapshot_dialog: None,
            paused: false,
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        let mut controls = Vec::new();

        // Tab input mode
        controls.push(Control::new("i", "Input"));

        // Pause (HumanVsEngine or EngineVsEngine) - but actual pause state comes from game session
        if matches!(
            self.game_mode,
            GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
        ) {
            controls.push(Control::new("p", "Pause"));
        }

        // Undo (if moves have been played)
        if self.move_count > 0 {
            controls.push(Control::new("u", "Undo"));
        }

        // Menu
        controls.push(Control::new("Esc", "Menu"));

        // Panels navigation
        controls.push(Control::new("Tab", "Panels"));
        controls.push(Control::new("h/l", "Section"));
        controls.push(Control::new("j/k", "Next"));
        controls.push(Control::new("S+j/k", "Scroll"));

        // UCI debug
        controls.push(Control::new("@", "UCI"));

        // Engine panel
        controls.push(Control::new("#", "Engine"));

        // Quit
        controls.push(Control::new("Ctrl+C", "Quit"));

        controls
    }

    /// Derive layout based on current state
    /// Takes shared UiStateMachine for accessing pane_manager, tab_input, etc.
    pub fn layout(&self, shared: &crate::ui::fsm::UiStateMachine) -> Layout {
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
        if shared.component_manager.is_visible(&Component::EnginePanel) {
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
