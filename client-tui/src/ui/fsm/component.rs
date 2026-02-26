use crate::state::GameSession;
use crate::ui::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Component {
    Board,
    TabInput,
    Controls,
    InfoPanel,
    HistoryPanel,
    EnginePanel,
    DebugPanel,
    ReviewSummary,
    AdvancedAnalysis,
}

pub struct ComponentProperties {
    #[allow(dead_code)] // structural; used when component API is fully extended (e.g. panel lists)
    pub component: Component,
    pub title: &'static str,
    pub is_selectable: bool,
    pub is_expandable: bool,
    #[allow(dead_code)] // border_color now resolved from Theme in panel_state()
    pub border_color: Color,
}

impl ComponentProperties {
    pub fn for_component(component: &Component) -> ComponentProperties {
        match component {
            Component::Board => ComponentProperties {
                component: Component::Board,
                title: "Board",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Cyan,
            },
            Component::TabInput => ComponentProperties {
                component: Component::TabInput,
                title: "Tab Input",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Cyan,
            },
            Component::Controls => ComponentProperties {
                component: Component::Controls,
                title: "Controls",
                is_selectable: false,
                is_expandable: false,
                border_color: Color::Cyan,
            },
            Component::InfoPanel => ComponentProperties {
                component: Component::InfoPanel,
                title: "Game Info",
                is_selectable: true,
                is_expandable: false,
                border_color: Color::Cyan,
            },
            Component::HistoryPanel => ComponentProperties {
                component: Component::HistoryPanel,
                title: "Move History",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Cyan,
            },
            Component::EnginePanel => ComponentProperties {
                component: Component::EnginePanel,
                title: "Engine Analysis",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Cyan,
            },
            Component::DebugPanel => ComponentProperties {
                component: Component::DebugPanel,
                title: "UCI Debug",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Magenta,
            },
            Component::ReviewSummary => ComponentProperties {
                component: Component::ReviewSummary,
                title: "Review Summary",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Green,
            },
            Component::AdvancedAnalysis => ComponentProperties {
                component: Component::AdvancedAnalysis,
                title: "Advanced Analysis",
                is_selectable: true,
                is_expandable: true,
                border_color: Color::Magenta,
            },
        }
    }
}

/// Bundled panel state computed from the FSM for rendering.
/// Provides all the common state every panel widget needs.
pub struct PanelState {
    #[allow(dead_code)] // structural; used when panel list API is extended
    pub component: Component,
    pub title: &'static str,
    pub number_key_hint: Option<char>,
    pub is_selected: bool,
    pub scroll: u16,
    pub expanded: bool,
    pub border_color: Color,
    /// When true, this is the sidebar instance of an expanded panel.
    /// Chrome renders dimmed and content is skipped.
    pub dimmed: bool,
}

impl PanelState {
    /// Render the shared panel chrome (titled border with selection/expanded indicators)
    /// and return the inner `Rect` for content rendering.
    ///
    /// Accepts an optional `suffix` for dynamic text appended to the title
    /// (e.g. " (Thinking...)" for the engine panel).
    pub fn render_chrome(&self, area: Rect, buf: &mut Buffer, suffix: &str, theme: &Theme) -> Rect {
        let base_title = if self.is_selected {
            format!("{} [SELECTED]{}", self.title, suffix)
        } else {
            format!(
                "[{}] {}{}",
                self.number_key_hint.unwrap_or(' '),
                self.title,
                suffix
            )
        };
        let title = if self.expanded {
            format!("{} (Expanded)", base_title)
        } else {
            base_title
        };

        let border_style = if self.dimmed {
            Style::default().fg(theme.panel_border_dimmed)
        } else if self.is_selected || self.expanded {
            Style::default()
                .fg(theme.panel_border_selected)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.border_color)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);
        inner
    }
}

impl Component {
    pub fn properties(&self) -> ComponentProperties {
        ComponentProperties::for_component(self)
    }

    pub fn is_selectable(&self) -> bool {
        self.properties().is_selectable
    }

    pub fn is_expandable(&self) -> bool {
        self.properties().is_expandable
    }

    /// Returns the number key ('1'-'4') assigned to this component for direct selection
    /// in the given UI mode, or `None` if this component is not selectable via number key.
    ///
    /// Game mode:   1=InfoPanel, 2=EnginePanel, 3=HistoryPanel, 4=DebugPanel
    /// Review mode: 1=InfoPanel, 2=HistoryPanel, 3=AdvancedAnalysis, 4=ReviewSummary
    pub fn number_key(&self, mode: &super::UiMode) -> Option<char> {
        match (self, mode) {
            (Component::InfoPanel, _) => Some('1'),
            (Component::EnginePanel, _) => Some('2'),
            (Component::HistoryPanel, super::UiMode::ReviewBoard) => Some('2'),
            (Component::HistoryPanel, _) => Some('3'),
            (Component::DebugPanel, _) => Some('4'),
            (Component::AdvancedAnalysis, _) => Some('3'),
            (Component::ReviewSummary, _) => Some('4'),
            _ => None,
        }
    }

    pub fn panel_state(&self, fsm: &super::UiStateMachine) -> PanelState {
        let props = self.properties();
        let theme = &fsm.context.theme;
        let border_color = match self {
            Component::DebugPanel | Component::AdvancedAnalysis => theme.panel_border_debug,
            Component::ReviewSummary => theme.panel_border_review,
            _ => theme.panel_border,
        };
        PanelState {
            component: *self,
            title: props.title,
            number_key_hint: self.number_key(&fsm.mode),
            is_selected: fsm.selected_component() == Some(*self),
            scroll: fsm.component_scroll(self),
            expanded: fsm.expanded_component() == Some(*self),
            border_color,
            dimmed: false, // set by renderer when this is the sidebar duplicate
        }
    }

    /// Whether this component is a panel (has chrome border/title).
    pub fn has_chrome(&self) -> bool {
        !matches!(
            self,
            Component::Board | Component::TabInput | Component::Controls
        )
    }

    /// Whether this panel should render given the current game state.
    /// Review panels require an active review_state.
    pub fn should_render(&self, game_session: &GameSession) -> bool {
        match self {
            Component::ReviewSummary | Component::AdvancedAnalysis => {
                game_session.review_state.is_some()
            }
            _ => true,
        }
    }

    /// Dynamic suffix appended to the chrome title bar.
    pub fn chrome_suffix(&self, game_session: &GameSession) -> &str {
        match self {
            Component::EnginePanel if game_session.is_engine_thinking => " (Thinking...)",
            _ => "",
        }
    }

    /// Render the panel's content widget into the given area.
    /// Assumes chrome has already been rendered and `area` is the inner rect.
    pub fn render_content(
        &self,
        area: Rect,
        buf: &mut Buffer,
        game_session: &GameSession,
        fsm: &super::UiStateMachine,
        ps: &PanelState,
    ) {
        use crate::ui::widgets::{
            advanced_analysis_panel::AdvancedAnalysisPanel, engine_panel::EngineAnalysisPanel,
            game_info_panel::GameInfoPanel, move_history_panel::MoveHistoryPanel,
            review_summary_panel::ReviewSummaryPanel, uci_debug_panel::UciDebugPanel,
        };

        let theme = &fsm.context.theme;

        match self {
            Component::InfoPanel => {
                let widget = GameInfoPanel::new(game_session, fsm, ps.scroll, theme);
                widget.render(area, buf);
            }
            Component::HistoryPanel => {
                let review_positions = game_session
                    .review_state
                    .as_ref()
                    .map(|rs| rs.review.positions.as_slice());
                let current_ply = game_session.review_state.as_ref().map(|rs| rs.current_ply);
                let widget =
                    MoveHistoryPanel::new(game_session.history(), ps.scroll, ps.expanded, theme)
                        .with_review_positions(review_positions)
                        .with_current_ply(current_ply);
                widget.render(area, buf);
            }
            Component::EnginePanel => {
                let widget = EngineAnalysisPanel::new(
                    game_session.engine_info.as_ref(),
                    game_session.is_engine_thinking,
                    ps.scroll,
                    theme,
                );
                widget.render(area, buf);
            }
            Component::DebugPanel => {
                let widget = UciDebugPanel::new(&game_session.uci_log, ps.scroll, theme);
                widget.render(area, buf);
            }
            Component::ReviewSummary => {
                if let Some(ref review_state) = game_session.review_state {
                    let widget = ReviewSummaryPanel {
                        review_state,
                        scroll: ps.scroll,
                        theme,
                    };
                    widget.render(area, buf);
                }
            }
            Component::AdvancedAnalysis => {
                if let Some(ref review_state) = game_session.review_state {
                    let widget = AdvancedAnalysisPanel {
                        review_state,
                        scroll: ps.scroll,
                        theme,
                    };
                    widget.render(area, buf);
                }
            }
            _ => {} // Non-panel components handled by renderer directly
        }
    }

    /// Reverse lookup: resolve a number key to a Component for the given UI mode.
    ///
    /// Returns `None` if the key does not map to any component in the given mode.
    pub fn from_number_key(key: char, mode: &super::UiMode) -> Option<Component> {
        match (key, mode) {
            ('1', _) => Some(Component::InfoPanel),
            ('2', super::UiMode::ReviewBoard) => Some(Component::HistoryPanel),
            ('2', _) => Some(Component::EnginePanel),
            ('3', super::UiMode::ReviewBoard) => Some(Component::AdvancedAnalysis),
            ('3', _) => Some(Component::HistoryPanel),
            ('4', super::UiMode::ReviewBoard) => Some(Component::ReviewSummary),
            ('4', _) => Some(Component::DebugPanel),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::fsm::UiMode;

    #[test]
    fn number_key_game_mode_mapping() {
        let mode = UiMode::GameBoard;
        assert_eq!(Component::InfoPanel.number_key(&mode), Some('1'));
        assert_eq!(Component::EnginePanel.number_key(&mode), Some('2'));
        assert_eq!(Component::HistoryPanel.number_key(&mode), Some('3'));
        assert_eq!(Component::DebugPanel.number_key(&mode), Some('4'));
    }

    #[test]
    fn number_key_review_mode_mapping() {
        let mode = UiMode::ReviewBoard;
        assert_eq!(Component::InfoPanel.number_key(&mode), Some('1'));
        assert_eq!(Component::HistoryPanel.number_key(&mode), Some('2'));
        assert_eq!(Component::AdvancedAnalysis.number_key(&mode), Some('3'));
        assert_eq!(Component::ReviewSummary.number_key(&mode), Some('4'));
    }

    #[test]
    fn non_selectable_components_have_no_number_key() {
        let mode = UiMode::GameBoard;
        assert_eq!(Component::Board.number_key(&mode), None);
        assert_eq!(Component::TabInput.number_key(&mode), None);
        assert_eq!(Component::Controls.number_key(&mode), None);
    }

    #[test]
    fn from_number_key_game_board() {
        let mode = UiMode::GameBoard;
        assert_eq!(
            Component::from_number_key('1', &mode),
            Some(Component::InfoPanel)
        );
        assert_eq!(
            Component::from_number_key('2', &mode),
            Some(Component::EnginePanel)
        );
        assert_eq!(
            Component::from_number_key('3', &mode),
            Some(Component::HistoryPanel)
        );
        assert_eq!(
            Component::from_number_key('4', &mode),
            Some(Component::DebugPanel)
        );
    }

    #[test]
    fn from_number_key_review_board() {
        let mode = UiMode::ReviewBoard;
        assert_eq!(
            Component::from_number_key('1', &mode),
            Some(Component::InfoPanel)
        );
        assert_eq!(
            Component::from_number_key('2', &mode),
            Some(Component::HistoryPanel)
        );
        assert_eq!(
            Component::from_number_key('3', &mode),
            Some(Component::AdvancedAnalysis)
        );
        assert_eq!(
            Component::from_number_key('4', &mode),
            Some(Component::ReviewSummary)
        );
    }

    #[test]
    fn from_number_key_invalid_keys_return_none() {
        let mode = UiMode::GameBoard;
        assert_eq!(Component::from_number_key('0', &mode), None);
        assert_eq!(Component::from_number_key('5', &mode), None);
        assert_eq!(Component::from_number_key('a', &mode), None);
    }

    #[test]
    fn number_key_round_trip_game_mode() {
        let mode = UiMode::GameBoard;
        for component in [
            Component::InfoPanel,
            Component::EnginePanel,
            Component::HistoryPanel,
            Component::DebugPanel,
        ] {
            let key = component.number_key(&mode).unwrap();
            assert_eq!(Component::from_number_key(key, &mode), Some(component));
        }
    }

    #[test]
    fn number_key_round_trip_review_mode() {
        let mode = UiMode::ReviewBoard;
        for component in [
            Component::InfoPanel,
            Component::HistoryPanel,
            Component::AdvancedAnalysis,
            Component::ReviewSummary,
        ] {
            let key = component.number_key(&mode).unwrap();
            assert_eq!(Component::from_number_key(key, &mode), Some(component));
        }
    }
}
