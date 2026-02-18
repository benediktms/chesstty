pub mod states;

pub use states::*;

pub mod hooks;
pub mod render_spec;
pub mod renderer;

use crate::ui::context::FocusStack;
use crate::ui::pane::PaneManager;
use render_spec::{InputPhase, Layout, RenderSpec, TabInputState};
use statig::prelude::*;

pub type AppStateMachine = UiStateMachine;

pub struct AppContext {
    pub server_address: String,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            server_address: "http://[::1]:50051".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum UiState {
    StartScreen(StartScreenState),
    GameBoard(GameBoardState),
    ReviewBoard(ReviewBoardState),
    MatchSummary(MatchSummaryState),
}

impl Default for UiState {
    fn default() -> Self {
        UiState::StartScreen(StartScreenState::new())
    }
}

impl UiState {
    pub fn start_screen() -> Self {
        UiState::StartScreen(StartScreenState::new())
    }

    pub fn render_spec(&self) -> &RenderSpec {
        match self {
            UiState::StartScreen(state) => &state.render_spec,
            UiState::GameBoard(state) => &state.render_spec,
            UiState::ReviewBoard(state) => &state.render_spec,
            UiState::MatchSummary(state) => &state.render_spec,
        }
    }

    /// Get the layout derived from current state
    /// Takes a reference to UiStateMachine for shared state access
    pub fn layout(&self, shared: &UiStateMachine) -> Layout {
        match self {
            UiState::StartScreen(state) => state.render_spec.layout.clone(),
            UiState::GameBoard(state) => state.layout(shared),
            UiState::ReviewBoard(state) => state.layout(shared),
            UiState::MatchSummary(state) => state.render_spec.layout.clone(),
        }
    }

    pub fn controls(&self) -> &Vec<crate::ui::fsm::render_spec::Control> {
        match self {
            UiState::StartScreen(state) => &state.controls,
            UiState::GameBoard(state) => &state.controls,
            UiState::ReviewBoard(state) => &state.controls,
            UiState::MatchSummary(state) => &state.controls,
        }
    }

    pub fn game_board() -> Self {
        UiState::GameBoard(GameBoardState::default())
    }

    pub fn review_board() -> Self {
        UiState::ReviewBoard(ReviewBoardState::default())
    }

    pub fn match_summary() -> Self {
        UiState::MatchSummary(MatchSummaryState::default())
    }
}

#[derive(Clone, Debug)]
pub enum UiEvent {
    Key(crossterm::event::KeyEvent),
    ServerEvent(chess_client::SessionStreamEvent),
    TimerTick,
    StartGame(crate::ui::menu_app::GameConfig),
    StartReview { game_id: String },
    ReturnToMenu,
    Quit,
}

impl Default for UiEvent {
    fn default() -> Self {
        UiEvent::TimerTick
    }
}

pub struct UiStateMachine {
    pub context: AppContext,
    // Current state indicator for layout delegation
    pub current_state: UiState,
    // UI state - shared across all FSM states (statig shared storage)
    pub pane_manager: PaneManager,
    pub tab_input: TabInputState,
    pub input_phase: InputPhase,
    pub focus_stack: FocusStack,
    pub popup_menu: Option<crate::ui::widgets::popup_menu::PopupMenuState>,
    pub snapshot_dialog: Option<crate::ui::widgets::snapshot_dialog::SnapshotDialogState>,
    pub review_tab: u8,
    pub review_moves_selection: Option<u32>,
    // Promotion dialog UI state
    pub selected_promotion_piece: cozy_chess::Piece,
}

impl Default for UiStateMachine {
    fn default() -> Self {
        Self {
            context: AppContext::default(),
            current_state: UiState::StartScreen(StartScreenState::new()),
            pane_manager: PaneManager::default(),
            tab_input: TabInputState::default(),
            input_phase: InputPhase::default(),
            focus_stack: FocusStack::default(),
            popup_menu: None,
            snapshot_dialog: None,
            review_tab: 0,
            review_moves_selection: None,
            selected_promotion_piece: cozy_chess::Piece::Queen,
        }
    }
}

#[state_machine(initial = "State::start_screen()")]
impl UiStateMachine {
    #[state]
    fn start_screen(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Enter => {
                        self.current_state = UiState::game_board();
                        self.setup_game_mode();
                        Outcome::Transition(State::game_board())
                    }
                    _ => Outcome::Handled,
                }
            }
            UiEvent::StartGame(config) => {
                if config.mode == crate::state::GameMode::ReviewMode {
                    self.current_state = UiState::review_board();
                    self.setup_review_mode();
                    Outcome::Transition(State::review_board())
                } else {
                    self.current_state = UiState::game_board();
                    self.setup_game_mode();
                    Outcome::Transition(State::game_board())
                }
            }
            UiEvent::StartReview { .. } => {
                self.current_state = UiState::review_board();
                self.setup_review_mode();
                Outcome::Transition(State::review_board())
            }
            UiEvent::Quit => {
                self.current_state = UiState::start_screen();
                Outcome::Transition(State::start_screen())
            }
            _ => Outcome::Handled,
        }
    }

    #[state]
    fn game_board(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(_) => Outcome::Handled,
            UiEvent::ReturnToMenu => {
                self.current_state = UiState::start_screen();
                Outcome::Transition(State::start_screen())
            }
            _ => Outcome::Handled,
        }
    }

    #[state]
    fn review_board(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(_) => Outcome::Handled,
            UiEvent::ReturnToMenu => {
                self.current_state = UiState::start_screen();
                Outcome::Transition(State::start_screen())
            }
            _ => Outcome::Handled,
        }
    }

    #[state]
    fn match_summary(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('n') | KeyCode::Enter => {
                        self.current_state = UiState::start_screen();
                        Outcome::Transition(State::start_screen())
                    }
                    KeyCode::Char('q') => {
                        self.current_state = UiState::start_screen();
                        Outcome::Transition(State::start_screen())
                    }
                    _ => Outcome::Handled,
                }
            }
            _ => Outcome::Handled,
        }
    }
}

impl UiStateMachine {
    /// Set up pane visibility for game mode
    pub fn setup_game_mode(&mut self) {
        use crate::ui::pane::PaneId;
        // Game mode: show GameInfo, EngineAnalysis, MoveHistory
        self.pane_manager.set_visible(PaneId::GameInfo, true);
        self.pane_manager.set_visible(PaneId::EngineAnalysis, true);
        self.pane_manager.set_visible(PaneId::MoveHistory, true);
        self.pane_manager.set_visible(PaneId::ReviewSummary, false);
        self.pane_manager
            .set_visible(PaneId::AdvancedAnalysis, false);
    }

    /// Set up pane visibility for review mode
    pub fn setup_review_mode(&mut self) {
        use crate::ui::pane::PaneId;
        // Review mode: show GameInfo, MoveHistory, ReviewSummary, AdvancedAnalysis
        // (GameInfo and MoveHistory are on the right, ReviewSummary and AdvancedAnalysis on left)
        self.pane_manager.set_visible(PaneId::GameInfo, true);
        self.pane_manager.set_visible(PaneId::EngineAnalysis, false);
        self.pane_manager.set_visible(PaneId::MoveHistory, true);
        self.pane_manager.set_visible(PaneId::ReviewSummary, true);
        self.pane_manager
            .set_visible(PaneId::AdvancedAnalysis, true);
    }

    /// Derive layout from current UI state
    /// Delegates to state-level layout methods
    pub fn layout(&self, game_session: &crate::state::GameSession) -> Layout {
        // Get the current state from our tracked state and delegate to its layout method
        let mut layout = self.current_state.layout(self);

        // Add overlay from shared state
        layout.overlay = self.derive_overlay();

        layout
    }

    /// Get the active overlay based on current UI state
    pub fn overlay(&self) -> render_spec::Overlay {
        self.derive_overlay()
    }

    fn derive_overlay(&self) -> render_spec::Overlay {
        use render_spec::Overlay;

        // Check for promotion dialog first
        if let InputPhase::SelectPromotion { from, to } = &self.input_phase {
            return Overlay::PromotionDialog {
                from: *from,
                to: *to,
            };
        }

        // Check for popup menu
        if self.popup_menu.is_some() {
            return Overlay::PopupMenu;
        }

        // Check for snapshot dialog
        if self.snapshot_dialog.is_some() {
            return Overlay::SnapshotDialog;
        }

        // Check for expanded panel via pane_manager
        if let Some(component) = self.pane_manager.expanded() {
            let comp = match component {
                crate::ui::pane::PaneId::GameInfo => render_spec::Component::InfoPanel,
                crate::ui::pane::PaneId::MoveHistory => render_spec::Component::HistoryPanel,
                crate::ui::pane::PaneId::EngineAnalysis => render_spec::Component::EnginePanel,
                crate::ui::pane::PaneId::UciDebug => render_spec::Component::DebugPanel,
                crate::ui::pane::PaneId::ReviewSummary => render_spec::Component::ReviewSummary,
                crate::ui::pane::PaneId::AdvancedAnalysis => {
                    render_spec::Component::AdvancedAnalysis
                }
            };
            return Overlay::ExpandedPanel { component: comp };
        }

        Overlay::None
    }

    /// Build board overlay from game session (for game mode)
    pub fn board_overlay(
        &self,
        game_session: &crate::state::GameSession,
    ) -> crate::ui::widgets::board_overlay::BoardOverlay {
        use crate::ui::widgets::board_overlay::{BoardOverlay, OverlayColor};

        let mut overlay = BoardOverlay::new();

        // Layer 1: Last move (lowest priority)
        if let Some((from, to)) = game_session.last_move {
            overlay.tint(from, OverlayColor::LastMove);
            overlay.tint(to, OverlayColor::LastMove);
        }

        // Layer 2: Best move (engine recommendation) - arrow and outline squares
        if let Some((from, to)) = game_session.best_move_squares {
            overlay.arrow(from, to, OverlayColor::BestMove);
            overlay.outline(from, OverlayColor::BestMove);
            overlay.outline(to, OverlayColor::BestMove);
        }

        // Layer 3: Legal move destinations (highlighted squares)
        for &sq in &game_session.highlighted_squares {
            overlay.tint(sq, OverlayColor::LegalMove);
        }

        // Layer 4: Selected piece (highest priority)
        if let Some(sq) = game_session.selected_square {
            overlay.tint(sq, OverlayColor::Selected);
        }

        overlay
    }
}
