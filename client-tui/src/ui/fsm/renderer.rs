use crate::state::{GameMode, GameSession, PlayerColor};
use crate::ui::fsm::render_spec::{Component, Constraint, Layout, Overlay, Row};
use crate::ui::fsm::UiStateMachine;
use crate::ui::widgets::{
    advanced_analysis_panel::AdvancedAnalysisPanel, board_overlay::build_review_overlay,
    review_summary_panel::ReviewSummaryPanel, review_tabs_panel::ReviewTabsPanel, BoardWidget,
};
use ratatui::{layout::Rect, Frame};

pub struct Renderer;

impl Renderer {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        layout: &Layout,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        let row_areas = Self::split_vertical(area, &layout.rows);

        for (row, row_area) in layout.rows.iter().zip(row_areas.iter()) {
            let section_areas = Self::split_horizontal(*row_area, &row.sections);

            for (section, section_area) in row.sections.iter().zip(section_areas.iter()) {
                Self::render_section_content(
                    frame,
                    *section_area,
                    &section.content,
                    game_session,
                    fsm,
                );
            }
        }

        let overlay = fsm.overlay();
        if !matches!(overlay, Overlay::None) {
            Self::render_overlay(frame, area, overlay, game_session, fsm);
        }
    }

    fn split_vertical(area: Rect, rows: &[Row]) -> Vec<Rect> {
        if rows.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = rows
            .iter()
            .map(|r| Self::to_constraint(&r.height))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn split_horizontal(
        area: Rect,
        sections: &[crate::ui::fsm::render_spec::Section],
    ) -> Vec<Rect> {
        if sections.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = sections
            .iter()
            .map(|c| Self::to_constraint(&c.constraint))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn split_vertical_nested(
        area: Rect,
        sections: &[crate::ui::fsm::render_spec::Section],
    ) -> Vec<Rect> {
        if sections.is_empty() {
            return vec![];
        }

        let constraints: Vec<ratatui::layout::Constraint> = sections
            .iter()
            .map(|c| Self::to_constraint(&c.constraint))
            .collect();

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    fn to_constraint(c: &Constraint) -> ratatui::layout::Constraint {
        match c {
            Constraint::Length(n) => ratatui::layout::Constraint::Length(*n),
            Constraint::Min(n) => ratatui::layout::Constraint::Min(*n),
            Constraint::Percentage(p) => ratatui::layout::Constraint::Percentage(*p),
            Constraint::Ratio(n, d) => ratatui::layout::Constraint::Ratio((*n).into(), (*d).into()),
        }
    }

    fn render_section_content(
        frame: &mut Frame,
        area: Rect,
        content: &crate::ui::fsm::render_spec::SectionContent,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        match content {
            crate::ui::fsm::render_spec::SectionContent::Component(component) => {
                Self::render_component(frame, area, component, game_session, fsm);
            }
            crate::ui::fsm::render_spec::SectionContent::Nested(sections) => {
                let section_areas = Self::split_vertical_nested(area, sections);
                for (section, section_area) in sections.iter().zip(section_areas.iter()) {
                    Self::render_section_content(
                        frame,
                        *section_area,
                        &section.content,
                        game_session,
                        fsm,
                    );
                }
            }
        }
    }

    fn render_component(
        frame: &mut Frame,
        area: Rect,
        component: &Component,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        use crate::ui::widgets::{
            EngineAnalysisPanel, GameInfoPanel, MoveHistoryPanel, TabInputWidget, UciDebugPanel,
        };

        match component {
            Component::Board => {
                let is_flipped = matches!(
                    game_session.mode,
                    GameMode::HumanVsEngine {
                        human_side: PlayerColor::Black
                    }
                );
                let board_overlay = if let Some(ref review) = game_session.review_state {
                    build_review_overlay(review)
                } else {
                    fsm.board_overlay(game_session)
                };
                let board_widget = BoardWidget {
                    board: game_session.board(),
                    overlay: &board_overlay,
                    flipped: is_flipped,
                };
                frame.render_widget(board_widget, area);
            }
            Component::TabInput => {
                let widget = TabInputWidget::new(game_session, fsm);
                frame.render_widget(widget, area);
            }
            Component::Controls => {
                use ratatui::style::{Color, Modifier, Style};
                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let controls = fsm.derive_controls(game_session);
                let key_style = Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD);
                let alert_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

                let mut spans = Vec::new();
                for (i, control) in controls.iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::raw(" | "));
                    }

                    // Special styling for alert-style keys (PAUSED, Ctrl+C)
                    let style = if control.key == "PAUSED" || control.key == "Ctrl+C" {
                        alert_style
                    } else {
                        key_style
                    };

                    spans.push(Span::styled(control.key, style));
                    if !control.label.is_empty() {
                        spans.push(Span::raw(format!(" {}", control.label)));
                    }
                }

                let controls_line =
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
                frame.render_widget(controls_line, area);
            }
            Component::InfoPanel => {
                let ps = component.panel_state(fsm);
                let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                let widget = GameInfoPanel::new(game_session, fsm, ps.scroll);
                frame.render_widget(widget, inner);
            }
            Component::HistoryPanel => {
                let ps = component.panel_state(fsm);
                let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                let review_positions = game_session
                    .review_state
                    .as_ref()
                    .map(|rs| rs.review.positions.as_slice());
                let current_ply = game_session.review_state.as_ref().map(|rs| rs.current_ply);
                let widget = MoveHistoryPanel::new(game_session.history(), ps.scroll, ps.expanded)
                    .with_review_positions(review_positions)
                    .with_current_ply(current_ply);
                frame.render_widget(widget, inner);
            }
            Component::EnginePanel => {
                let ps = component.panel_state(fsm);
                let suffix = if game_session.is_engine_thinking { " (Thinking...)" } else { "" };
                let inner = ps.render_chrome(area, frame.buffer_mut(), suffix);
                let widget = EngineAnalysisPanel::new(
                    game_session.engine_info.as_ref(),
                    game_session.is_engine_thinking,
                    ps.scroll,
                );
                frame.render_widget(widget, inner);
            }
            Component::DebugPanel => {
                let ps = component.panel_state(fsm);
                let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                let widget = UciDebugPanel::new(&game_session.uci_log, ps.scroll);
                frame.render_widget(widget, inner);
            }
            Component::ReviewTabs => {
                if let Some(ref review_state) = game_session.review_state {
                    let ps = component.panel_state(fsm);
                    let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                    // ReviewTabs reads scroll from ReviewSummary's state (quirk)
                    let scroll = fsm.component_scroll(&Component::ReviewSummary);
                    let widget = ReviewTabsPanel {
                        review_state,
                        current_tab: fsm.review_tab,
                        scroll,
                        moves_selection: None,
                    };
                    frame.render_widget(widget, inner);
                }
            }
            Component::ReviewSummary => {
                if let Some(ref review_state) = game_session.review_state {
                    let ps = component.panel_state(fsm);
                    let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                    let widget = ReviewSummaryPanel {
                        review_state,
                        scroll: ps.scroll,
                    };
                    frame.render_widget(widget, inner);
                }
            }
            Component::AdvancedAnalysis => {
                if let Some(ref review_state) = game_session.review_state {
                    let ps = component.panel_state(fsm);
                    let inner = ps.render_chrome(area, frame.buffer_mut(), "");
                    let widget = AdvancedAnalysisPanel {
                        review_state,
                        scroll: ps.scroll,
                    };
                    frame.render_widget(widget, inner);
                }
            }
        }
    }

    fn render_overlay(
        frame: &mut Frame,
        area: Rect,
        overlay: Overlay,
        _game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        use crate::ui::widgets::{PopupMenuWidget, PromotionWidget, SnapshotDialogWidget};

        match overlay {
            Overlay::None => {}
            Overlay::PopupMenu => {
                if let Some(ref state) = fsm.popup_menu {
                    let widget = PopupMenuWidget { state };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::SnapshotDialog => {
                if let Some(ref state) = fsm.snapshot_dialog {
                    let widget = SnapshotDialogWidget { state };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::PromotionDialog { .. } => {
                let widget = PromotionWidget {
                    selected_piece: fsm.selected_promotion_piece,
                };
                frame.render_widget(widget, area);
            }
        }
    }
}
