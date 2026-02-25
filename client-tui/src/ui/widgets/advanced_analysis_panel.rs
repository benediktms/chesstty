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

        let advanced = match &self.review_state.advanced {
            Some(a) => a,
            None => {
                lines.push(Line::from(Span::styled(
                    "No advanced analysis available",
                    Style::default().fg(Color::DarkGray),
                )));
                let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
                paragraph.render(area, buf);
                return;
            }
        };

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

        // Game-wide analysis (static)
        lines.push(Line::from(Span::styled(
            "Game-wide Analysis",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        // Phase performance
        lines.push(Line::from(Span::styled(
            "Phase Performance (avg cp_loss)",
            Style::default().fg(Color::Cyan),
        )));

        let white_psy = advanced.white_psychology.as_ref();
        let black_psy = advanced.black_psychology.as_ref();

        let w_opening = white_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
        let b_opening = black_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
        let w_mid = white_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
        let b_mid = black_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
        let w_end = white_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);
        let b_end = black_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);

        let w_max_err = white_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
        let b_max_err = black_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
        let w_blunder = white_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);
        let b_blunder = black_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);

        let w_fav = white_psy.map(|p| p.favorable_swings).unwrap_or(0);
        let b_fav = black_psy.map(|p| p.favorable_swings).unwrap_or(0);
        let w_unfav = white_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
        let b_unfav = black_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
        let w_streak = white_psy.map(|p| p.max_momentum_streak).unwrap_or(0);
        let b_streak = black_psy.map(|p| p.max_momentum_streak).unwrap_or(0);

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Opening", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_opening),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_opening),
                Style::default().fg(Color::LightCyan),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Middlegame", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_mid),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_mid),
                Style::default().fg(Color::LightCyan),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Endgame", Style::default().fg(Color::White)),
            Span::raw(": W "),
            Span::styled(
                format!("{:.1}", w_end),
                Style::default().fg(Color::LightCyan),
            ),
            Span::raw("  B "),
            Span::styled(
                format!("{:.1}", b_end),
                Style::default().fg(Color::LightCyan),
            ),
        ]));

        lines.push(Line::raw(""));

        // Error patterns
        lines.push(Line::from(Span::styled(
            "Error Patterns",
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(vec![
            Span::raw("  Max consecutive: "),
            Span::styled(
                format!("W:{}  B:{}", w_max_err, b_max_err),
                Style::default().fg(Color::LightRed),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Blunder cluster: "),
            Span::styled(
                format!("W:{}  B:{}", w_blunder, b_blunder),
                Style::default().fg(Color::LightMagenta),
            ),
        ]));

        lines.push(Line::raw(""));

        // Momentum
        lines.push(Line::from(Span::styled(
            "Momentum",
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(vec![
            Span::raw("  Favorable swings: "),
            Span::styled(
                format!("W:{}  B:{}", w_fav, b_fav),
                Style::default().fg(Color::Green),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Unfavorable swings: "),
            Span::styled(
                format!("W:{}  B:{}", w_unfav, b_unfav),
                Style::default().fg(Color::Red),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Max streak: "),
            Span::styled(
                format!("W:{}  B:{}", w_streak, b_streak),
                Style::default().fg(Color::Yellow),
            ),
        ]));

        lines.push(Line::raw(""));

        // Info
        lines.push(Line::from(vec![
            Span::styled("Critical positions: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", advanced.critical_positions_count)),
        ]));

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

