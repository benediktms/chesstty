pub mod states;

pub use states::*;

pub mod component;
pub use component::Component;
pub mod hooks;
pub mod render_spec;
pub mod renderer;

use std::collections::HashMap;

use render_spec::{Control, InputPhase, Layout, Section, SectionContent, TabInputState};

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

#[derive(Clone, Debug, PartialEq)]
pub enum UiMode {
    StartScreen,
    GameBoard,
    ReviewBoard,
    MatchSummary,
}

impl Default for UiMode {
    fn default() -> Self {
        UiMode::StartScreen
    }
}

pub struct UiStateMachine {
    pub context: AppContext,
    pub mode: UiMode,
    pub tab_input: TabInputState,
    pub input_phase: InputPhase,
    pub popup_menu: Option<crate::ui::widgets::popup_menu::PopupMenuState>,
    pub snapshot_dialog: Option<crate::ui::widgets::snapshot_dialog::SnapshotDialogState>,
    pub review_tab: u8,
    pub review_moves_selection: Option<u32>,
    pub selected_promotion_piece: cozy_chess::Piece,
    pub focused_component: Option<Component>,
    pub expanded: bool,
    pub visibility: HashMap<Component, bool>,
    pub scroll_state: HashMap<Component, u16>,
}

impl Default for UiStateMachine {
    fn default() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert(Component::InfoPanel, true);
        visibility.insert(Component::HistoryPanel, true);
        visibility.insert(Component::EnginePanel, true);
        visibility.insert(Component::ReviewSummary, false);
        visibility.insert(Component::AdvancedAnalysis, false);
        visibility.insert(Component::DebugPanel, false);

        let mut scroll_state = HashMap::new();
        scroll_state.insert(Component::InfoPanel, 0);
        scroll_state.insert(Component::HistoryPanel, 0);
        scroll_state.insert(Component::EnginePanel, 0);
        scroll_state.insert(Component::DebugPanel, 0);
        scroll_state.insert(Component::ReviewSummary, 0);
        scroll_state.insert(Component::AdvancedAnalysis, 0);

        Self {
            context: AppContext::default(),
            mode: UiMode::StartScreen,
            tab_input: TabInputState::default(),
            input_phase: InputPhase::default(),
            popup_menu: None,
            snapshot_dialog: None,
            review_tab: 0,
            review_moves_selection: None,
            selected_promotion_piece: cozy_chess::Piece::Queen,
            focused_component: None,
            expanded: false,
            visibility,
            scroll_state,
        }
    }
}

impl UiStateMachine {
    /// Transition to a new UI mode, applying any mode-specific setup.
    pub fn transition_to(&mut self, mode: UiMode) {
        self.mode = mode;
        match &self.mode {
            UiMode::GameBoard => self.setup_game_mode(),
            UiMode::ReviewBoard => self.setup_review_mode(),
            _ => {}
        }
    }
}

impl UiStateMachine {
    /// Set up pane visibility for game mode
    pub fn setup_game_mode(&mut self) {
        self.visibility.insert(Component::InfoPanel, true);
        self.visibility.insert(Component::EnginePanel, true);
        self.visibility.insert(Component::HistoryPanel, true);
        self.visibility.insert(Component::ReviewSummary, false);
        self.visibility.insert(Component::AdvancedAnalysis, false);
    }

    /// Set up pane visibility for review mode
    pub fn setup_review_mode(&mut self) {
        self.visibility.insert(Component::InfoPanel, true);
        self.visibility.insert(Component::EnginePanel, false);
        self.visibility.insert(Component::HistoryPanel, true);
        self.visibility.insert(Component::ReviewSummary, true);
        self.visibility.insert(Component::AdvancedAnalysis, true);
    }

    /// Derive layout from current UI mode.
    pub fn layout(&self, _game_session: &crate::state::GameSession) -> Layout {
        let mut layout = match self.mode {
            UiMode::StartScreen => Layout::start_screen(),
            UiMode::GameBoard => GameBoardState.layout(self),
            UiMode::ReviewBoard => ReviewBoardState.layout(self),
            UiMode::MatchSummary => Layout::match_summary(),
        };

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

    /// Derive controls for the current UI mode and game state.
    /// Single source of truth for the controls bar at the bottom of the screen.
    pub fn derive_controls(&self, game_session: &crate::state::GameSession) -> Vec<Control> {
        use crate::state::GameMode;

        match self.mode {
            UiMode::StartScreen => {
                vec![Control::new("Enter", "Select")]
            }
            UiMode::MatchSummary => {
                vec![
                    Control::new("n", "New Game"),
                    Control::new("Enter", "Menu"),
                    Control::new("q", "Quit"),
                ]
            }
            UiMode::ReviewBoard => {
                vec![
                    Control::new("1-4", "Panels"),
                    Control::new("j/k", "Moves"),
                    Control::new("Space", "Auto"),
                    Control::new("Home/End", "Jump"),
                    Control::new("Esc", "Menu"),
                ]
            }
            UiMode::GameBoard => {
                let mut controls = vec![Control::new("i", "Input")];

                if matches!(
                    game_session.mode,
                    GameMode::HumanVsEngine { .. } | GameMode::EngineVsEngine
                ) {
                    if game_session.paused {
                        controls.push(Control::new("PAUSED", ""));
                    }
                    controls.push(Control::new("p", "Pause"));
                }

                if game_session.is_undo_allowed() {
                    controls.push(Control::new("u", "Undo"));
                }

                controls.push(Control::new("Esc", "Menu"));
                let panel_hint = if self.is_component_visible(&Component::DebugPanel) {
                    "1-4"
                } else {
                    "1-3"
                };
                controls.push(Control::new(panel_hint, "Panels"));
                controls.push(Control::new("@", "UCI"));
                controls.push(Control::new("Ctrl+C", "Quit"));

                controls
            }
        }
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

impl UiStateMachine {
    pub fn is_board_focused(&self) -> bool {
        self.focused_component.is_none()
    }

    pub fn selected_component(&self) -> Option<Component> {
        if !self.expanded {
            self.focused_component
        } else {
            None
        }
    }

    pub fn expanded_component(&self) -> Option<Component> {
        if self.expanded {
            self.focused_component
        } else {
            None
        }
    }

    pub fn select_component(&mut self, component: Component) {
        self.focused_component = Some(component);
        self.expanded = false;
    }

    pub fn expand_component(&mut self, component: Component) {
        self.focused_component = Some(component);
        self.expanded = true;
    }

    pub fn clear_focus(&mut self) -> bool {
        if self.focused_component.is_some() {
            self.focused_component = None;
            self.expanded = false;
            true
        } else {
            false
        }
    }

    pub fn is_component_visible(&self, component: &Component) -> bool {
        self.visibility.get(component).copied().unwrap_or(false)
    }

    pub fn set_component_visible(&mut self, component: Component, visible: bool) {
        self.visibility.insert(component, visible);
    }

    pub fn toggle_component_visibility(&mut self, component: Component) {
        let current = self.visibility.get(&component).copied().unwrap_or(false);
        self.visibility.insert(component, !current);
    }

    pub fn component_scroll(&self, component: &Component) -> u16 {
        self.scroll_state.get(component).copied().unwrap_or(0)
    }

    pub fn component_scroll_mut(&mut self, component: &Component) -> &mut u16 {
        self.scroll_state.entry(component.clone()).or_insert(0)
    }

    // Navigation methods

    fn flatten_sections(&self, sections: &[Section]) -> Vec<Component> {
        let mut result = Vec::new();
        for section in sections {
            match &section.content {
                SectionContent::Component(component) => {
                    if component.is_selectable() && self.is_component_visible(component) {
                        result.push(*component);
                    }
                }
                SectionContent::Nested(nested_sections) => {
                    result.extend(self.flatten_sections(nested_sections));
                }
            }
        }
        result
    }

    pub fn tab_order(&self, layout: &Layout) -> Vec<Component> {
        let mut result = Vec::new();
        for row in &layout.rows {
            result.extend(self.flatten_sections(&row.sections));
        }
        result
    }

    fn visible_selectable_components(&self, layout: &Layout) -> Vec<Component> {
        self.tab_order(layout)
    }

    pub fn next_component(&self, current: Component, layout: &Layout) -> Option<Component> {
        let selectable = self.visible_selectable_components(layout);
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + 1) % selectable.len()]),
            None => selectable.first().copied(),
        }
    }

    pub fn prev_component(&self, current: Component, layout: &Layout) -> Option<Component> {
        let selectable = self.visible_selectable_components(layout);
        if selectable.is_empty() {
            return None;
        }
        let current_idx = selectable.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(selectable[(idx + selectable.len() - 1) % selectable.len()]),
            None => selectable.last().copied(),
        }
    }

    pub fn section_index(&self, component: Component, layout: &Layout) -> Option<usize> {
        let sections_by_pos = self.sections_by_position(layout);
        for (section_idx, sections) in sections_by_pos.into_iter().enumerate() {
            for section in sections {
                if self.section_contains_component(component, &section) {
                    return Some(section_idx);
                }
            }
        }
        None
    }

    pub fn components_in_section(&self, section_index: usize, layout: &Layout) -> Vec<Component> {
        let sections_by_pos = self.sections_by_position(layout);
        sections_by_pos
            .get(section_index)
            .map(|sections| self.flatten_sections(sections.as_slice()))
            .unwrap_or_default()
    }

    pub fn next_in_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let components = self.components_in_section(section_idx, layout);
        if components.is_empty() {
            return None;
        }
        let current_idx = components.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(components[(idx + 1) % components.len()]),
            None => components.first().copied(),
        }
    }

    pub fn prev_in_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let components = self.components_in_section(section_idx, layout);
        if components.is_empty() {
            return None;
        }
        let current_idx = components.iter().position(|c| *c == current);
        match current_idx {
            Some(idx) => Some(components[(idx + components.len() - 1) % components.len()]),
            None => components.last().copied(),
        }
    }

    pub fn next_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let sections_by_pos = self.sections_by_position(layout);
        let num_sections = sections_by_pos.len();
        if num_sections == 0 {
            return None;
        }
        let next_section_idx = (section_idx + 1) % num_sections;
        self.components_in_section(next_section_idx, layout)
            .into_iter()
            .next()
    }

    pub fn prev_section(&self, current: Component, layout: &Layout) -> Option<Component> {
        let section_idx = self.section_index(current, layout)?;
        let sections_by_pos = self.sections_by_position(layout);
        let num_sections = sections_by_pos.len();
        if num_sections == 0 {
            return None;
        }
        let prev_section_idx = if section_idx == 0 {
            num_sections - 1
        } else {
            section_idx - 1
        };
        self.components_in_section(prev_section_idx, layout)
            .into_iter()
            .next()
    }

    pub fn first_component(&self, layout: &Layout) -> Option<Component> {
        self.visible_selectable_components(layout)
            .into_iter()
            .next()
    }

    fn sections_by_position(&self, layout: &Layout) -> Vec<Vec<Section>> {
        let max_sections = layout
            .rows
            .iter()
            .map(|r| r.sections.len())
            .max()
            .unwrap_or(0);
        let mut result: Vec<Vec<Section>> = vec![Vec::new(); max_sections];
        let content_rows = layout.rows.len().saturating_sub(1);
        for row in layout.rows.iter().take(content_rows) {
            for (section_idx, section) in row.sections.iter().enumerate() {
                if section_idx < result.len() {
                    result[section_idx].push(section.clone());
                }
            }
        }
        result
    }

    fn section_contains_component(&self, component: Component, section: &Section) -> bool {
        match &section.content {
            SectionContent::Component(c) => *c == component,
            SectionContent::Nested(nested) => nested
                .iter()
                .any(|section| self.section_contains_component(component, section)),
        }
    }
}
