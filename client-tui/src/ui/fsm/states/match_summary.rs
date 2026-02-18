use crate::state::{GameMode, PlayerColor};
use crate::ui::context::FocusStack;
use crate::ui::fsm::render_spec::{Control, PaneState, RenderSpec};

#[derive(Clone, Debug)]
pub struct MatchSummaryState {
    pub pane_state: PaneState,
    pub game_result: Option<(i32, String)>,
    pub move_count: u32,
    pub game_mode: GameMode,
    pub winner: Option<PlayerColor>,
    pub render_spec: RenderSpec,
    pub controls: Vec<Control>,
    // UI state moved from RenderState
    pub focus_stack: FocusStack,
}

impl Default for MatchSummaryState {
    fn default() -> Self {
        let mut state = Self {
            pane_state: PaneState::match_summary(),
            game_result: None,
            move_count: 0,
            game_mode: GameMode::HumanVsHuman,
            winner: None,
            render_spec: RenderSpec::match_summary(),
            controls: Vec::new(),
            // UI state defaults
            focus_stack: FocusStack::default(),
        };
        state.controls = state.derive_controls();
        state
    }
}

impl MatchSummaryState {
    pub fn new(result: Option<(i32, String)>, move_count: u32, game_mode: GameMode) -> Self {
        let winner = result.as_ref().and_then(|(status, _)| {
            if *status == 1 {
                Some(PlayerColor::Black)
            } else {
                None
            }
        });

        let mut state = Self {
            pane_state: PaneState::match_summary(),
            game_result: result,
            move_count,
            game_mode,
            winner,
            render_spec: RenderSpec::match_summary(),
            controls: Vec::new(),
            // UI state defaults
            focus_stack: FocusStack::default(),
        };
        state.controls = state.derive_controls();
        state
    }

    pub fn derive_controls(&self) -> Vec<Control> {
        let mut controls = Vec::new();

        // New game
        controls.push(Control::new("n", "New Game"));

        // Return to menu
        controls.push(Control::new("Enter", "Menu"));

        // Quit
        controls.push(Control::new("q", "Quit"));

        controls
    }
}
