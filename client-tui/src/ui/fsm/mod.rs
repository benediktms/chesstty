pub mod states;

pub use states::*;

pub mod component;
pub use component::Component;
pub mod component_manager;
pub use component_manager::{ComponentManager, FocusMode};
pub mod hooks;
pub mod render_spec;
pub mod renderer;

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
    GameBoardPaneFocused(GameBoardPaneFocusedState),
    ReviewBoard(ReviewBoardState),
    ReviewBoardPaneFocused(ReviewBoardPaneFocusedState),
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
            UiState::GameBoardPaneFocused(state) => &state.render_spec,
            UiState::ReviewBoard(state) => &state.render_spec,
            UiState::ReviewBoardPaneFocused(state) => &state.render_spec,
            UiState::MatchSummary(state) => &state.render_spec,
        }
    }

    /// Get the layout derived from current state
    /// Takes a reference to UiStateMachine for shared state access
    pub fn layout(&self, shared: &UiStateMachine) -> Layout {
        match self {
            UiState::StartScreen(state) => state.render_spec.layout.clone(),
            UiState::GameBoard(state) => state.layout(shared),
            UiState::GameBoardPaneFocused(state) => state.layout(shared),
            UiState::ReviewBoard(state) => state.layout(shared),
            UiState::ReviewBoardPaneFocused(state) => state.layout(shared),
            UiState::MatchSummary(state) => state.render_spec.layout.clone(),
        }
    }

    pub fn controls(&self) -> &Vec<crate::ui::fsm::render_spec::Control> {
        match self {
            UiState::StartScreen(state) => &state.controls,
            UiState::GameBoard(state) => &state.controls,
            UiState::GameBoardPaneFocused(state) => &state.controls,
            UiState::ReviewBoard(state) => &state.controls,
            UiState::ReviewBoardPaneFocused(state) => &state.controls,
            UiState::MatchSummary(state) => &state.controls,
        }
    }

    pub fn game_board() -> Self {
        UiState::GameBoard(GameBoardState::default())
    }

    pub fn game_board_pane_focused(component: Component) -> Self {
        UiState::GameBoardPaneFocused(GameBoardPaneFocusedState::new(component))
    }

    pub fn review_board() -> Self {
        UiState::ReviewBoard(ReviewBoardState::default())
    }

    pub fn review_board_pane_focused(component: Component) -> Self {
        UiState::ReviewBoardPaneFocused(ReviewBoardPaneFocusedState::new(component))
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
    pub current_state: UiState,
    pub component_manager: ComponentManager,
    pub tab_input: TabInputState,
    pub input_phase: InputPhase,
    pub popup_menu: Option<crate::ui::widgets::popup_menu::PopupMenuState>,
    pub snapshot_dialog: Option<crate::ui::widgets::snapshot_dialog::SnapshotDialogState>,
    pub review_tab: u8,
    pub review_moves_selection: Option<u32>,
    pub selected_promotion_piece: cozy_chess::Piece,
}

impl Default for UiStateMachine {
    fn default() -> Self {
        Self {
            context: AppContext::default(),
            current_state: UiState::StartScreen(StartScreenState::new()),
            component_manager: ComponentManager::default(),
            tab_input: TabInputState::default(),
            input_phase: InputPhase::default(),
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
    fn game_board_pane_focused(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let UiState::GameBoardPaneFocused(state) = &mut self.current_state {
                            let scroll =
                                self.component_manager.scroll_mut(&state.focused_component);
                            *scroll = scroll.saturating_sub(5);
                        }
                        Outcome::Handled
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let UiState::GameBoardPaneFocused(state) = &mut self.current_state {
                            let scroll =
                                self.component_manager.scroll_mut(&state.focused_component);
                            *scroll = scroll.saturating_add(5);
                        }
                        Outcome::Handled
                    }
                    KeyCode::PageUp => {
                        if let UiState::GameBoardPaneFocused(state) = &mut self.current_state {
                            *self.component_manager.scroll_mut(&state.focused_component) = 0;
                        }
                        Outcome::Handled
                    }
                    KeyCode::PageDown => {
                        if let UiState::GameBoardPaneFocused(state) = &mut self.current_state {
                            *self.component_manager.scroll_mut(&state.focused_component) = u16::MAX;
                        }
                        Outcome::Handled
                    }
                    KeyCode::Esc => {
                        self.component_manager.clear_focus();
                        self.current_state = UiState::game_board();
                        Outcome::Transition(State::game_board())
                    }
                    _ => Outcome::Handled,
                }
            }
            _ => Outcome::Handled,
        }
    }

    #[state]
    fn review_board_pane_focused(&mut self, event: &UiEvent) -> Outcome<State> {
        match event {
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let UiState::ReviewBoardPaneFocused(state) = &mut self.current_state {
                            let scroll =
                                self.component_manager.scroll_mut(&state.focused_component);
                            *scroll = scroll.saturating_sub(5);
                        }
                        Outcome::Handled
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let UiState::ReviewBoardPaneFocused(state) = &mut self.current_state {
                            let scroll =
                                self.component_manager.scroll_mut(&state.focused_component);
                            *scroll = scroll.saturating_add(5);
                        }
                        Outcome::Handled
                    }
                    KeyCode::PageUp => {
                        if let UiState::ReviewBoardPaneFocused(state) = &mut self.current_state {
                            *self.component_manager.scroll_mut(&state.focused_component) = 0;
                        }
                        Outcome::Handled
                    }
                    KeyCode::PageDown => {
                        if let UiState::ReviewBoardPaneFocused(state) = &mut self.current_state {
                            *self.component_manager.scroll_mut(&state.focused_component) = u16::MAX;
                        }
                        Outcome::Handled
                    }
                    KeyCode::Esc => {
                        self.component_manager.clear_focus();
                        self.current_state = UiState::review_board();
                        Outcome::Transition(State::review_board())
                    }
                    _ => Outcome::Handled,
                }
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
        self.component_manager
            .set_visible(Component::InfoPanel, true);
        self.component_manager
            .set_visible(Component::EnginePanel, true);
        self.component_manager
            .set_visible(Component::HistoryPanel, true);
        self.component_manager
            .set_visible(Component::ReviewSummary, false);
        self.component_manager
            .set_visible(Component::AdvancedAnalysis, false);
    }

    /// Set up pane visibility for review mode
    pub fn setup_review_mode(&mut self) {
        self.component_manager
            .set_visible(Component::InfoPanel, true);
        self.component_manager
            .set_visible(Component::EnginePanel, false);
        self.component_manager
            .set_visible(Component::HistoryPanel, true);
        self.component_manager
            .set_visible(Component::ReviewSummary, true);
        self.component_manager
            .set_visible(Component::AdvancedAnalysis, true);
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
