use crate::state::{GameMode, GameSession, PlayerColor};
use crate::ui::fsm::render_spec::{Component, Constraint, Layout, Overlay, Row, Section};
use crate::ui::fsm::UiStateMachine;
use crate::ui::widgets::{board_overlay::build_review_overlay, BoardWidget};
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
                Self::render_section_content(frame, *section_area, section, game_session, fsm);
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
        section: &Section,
        game_session: &GameSession,
        fsm: &UiStateMachine,
    ) {
        match &section.content {
            crate::ui::fsm::render_spec::SectionContent::Component(component) => {
                Self::render_component(frame, area, component, game_session, fsm, section.dimmed);
            }
            crate::ui::fsm::render_spec::SectionContent::Nested(sections) => {
                let section_areas = Self::split_vertical_nested(area, sections);
                for (nested, section_area) in sections.iter().zip(section_areas.iter()) {
                    Self::render_section_content(frame, *section_area, nested, game_session, fsm);
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
        dimmed: bool,
    ) {
        use crate::ui::widgets::TabInputWidget;

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
                    theme: &fsm.context.theme,
                };
                frame.render_widget(board_widget, area);
            }
            Component::TabInput => {
                let widget = TabInputWidget::new(game_session, fsm);
                frame.render_widget(widget, area);
            }
            Component::Controls => {
                use ratatui::style::{Modifier, Style};
                use ratatui::text::{Line, Span};
                use ratatui::widgets::Paragraph;

                let theme = &fsm.context.theme;
                let controls = fsm.derive_controls(game_session);
                let key_style = Style::default()
                    .fg(theme.positive)
                    .add_modifier(Modifier::BOLD);
                let alert_style =
                    Style::default().fg(theme.negative).add_modifier(Modifier::BOLD);

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
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.dialog_bg));
                frame.render_widget(controls_line, area);
            }
            // Generic panel path: all chrome-bearing components share the same flow
            panel if panel.has_chrome() => {
                if !panel.should_render(game_session) {
                    return;
                }
                let mut ps = panel.panel_state(fsm);
                ps.dimmed = dimmed;
                let suffix = panel.chrome_suffix(game_session);
                let inner = ps.render_chrome(area, frame.buffer_mut(), suffix, &fsm.context.theme);
                if !dimmed {
                    panel.render_content(inner, frame.buffer_mut(), game_session, fsm, &ps);
                }
            }
            // Safety: all variants are covered above; Board/TabInput/Controls are non-chrome
            _ => {}
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

        let theme = &fsm.context.theme;

        match overlay {
            Overlay::None => {}
            Overlay::PopupMenu => {
                if let Some(ref state) = fsm.popup_menu {
                    let widget = PopupMenuWidget { state, theme };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::SnapshotDialog => {
                if let Some(ref state) = fsm.snapshot_dialog {
                    let widget = SnapshotDialogWidget { state, theme };
                    frame.render_widget(widget, area);
                }
            }
            Overlay::PromotionDialog { .. } => {
                let widget = PromotionWidget {
                    selected_piece: fsm.selected_promotion_piece,
                    theme,
                };
                frame.render_widget(widget, area);
            }
        }
    }
}
