use crate::review_state::ReviewState;
use crate::ui::widgets::review_helpers::{render_king_safety, render_tactical_tags, render_tension};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct AdvancedAnalysisPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
}

impl Widget for AdvancedAnalysisPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = vec![];

        if self.review_state.advanced.is_none() {
            lines.push(Line::from(Span::styled(
                "No advanced analysis available",
                Style::default().fg(Color::DarkGray),
            )));
            let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
            paragraph.render(area, buf);
            return;
        }

        // Per-position analysis (changes with ply navigation)
        if let Some(adv_pos) = self.review_state.advanced_position() {
            lines.push(Line::from(Span::styled(
                "Position Analysis",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            // Critical position badge
            if adv_pos.is_critical {
                lines.push(Line::from(Span::styled(
                    "  \u{26A0} CRITICAL POSITION \u{26A0}",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }

            // Tactical tags
            if !adv_pos.tactical_tags_after.is_empty() {
                render_tactical_tags(&mut lines, &adv_pos.tactical_tags_after);
            }

            // King safety
            if let Some(ref ks) = adv_pos.king_safety {
                render_king_safety(&mut lines, ks);
            }

            // Tension metrics
            if let Some(ref tension) = adv_pos.tension {
                render_tension(&mut lines, tension);
            }

            lines.push(Line::raw(""));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "Navigate to a move to see position analysis",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let content_height = lines.len() as u16;
        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(area, buf);

        if content_height > area.height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
            let mut scrollbar_state =
                ScrollbarState::new(content_height as usize).position(self.scroll as usize);
            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

