use crate::review_state::ReviewState;
use crate::ui::theme::Theme;
use crate::ui::widgets::review_helpers::{
    accuracy_bar, accuracy_color, build_eval_graph, count_classifications,
};
use chess::is_white_ply;
use chess_client::MoveClassification;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct ReviewSummaryPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
    pub theme: &'a Theme,
}

impl Widget for ReviewSummaryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = vec![];

        let review = &self.review_state.review;

        // Game winner at the very top
        if let Some(ref winner) = review.winner {
            let status_text = match winner.as_str() {
                "White" => "White Wins!",
                "Black" => "Black Wins!",
                "Draw" => "Draw",
                _ => "Unknown",
            };
            let status_color = match winner.as_str() {
                "White" => self.theme.text_primary,
                "Black" => self.theme.text_secondary,
                "Draw" => self.theme.warning,
                _ => self.theme.negative,
            };
            lines.push(Line::from(Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::raw(""));
        }

        // Accuracy section
        lines.push(Line::from(Span::styled(
            "Accuracy",
            Style::default()
                .fg(self.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(white_acc) = review.white_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  White: "),
                Span::styled(
                    format!("{:.1}%", white_acc),
                    Style::default().fg(accuracy_color(white_acc, self.theme)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(white_acc, 20)),
            ]));
        }
        if let Some(black_acc) = review.black_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  Black: "),
                Span::styled(
                    format!("{:.1}%", black_acc),
                    Style::default().fg(accuracy_color(black_acc, self.theme)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(black_acc, 20)),
            ]));
        }

        // Eval graph
        if !review.positions.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Evaluation",
                Style::default()
                    .fg(self.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            )));
            let graph_width = (area.width as usize).saturating_sub(4).min(60);
            let graph_lines = build_eval_graph(&review.positions, graph_width, self.theme);
            lines.extend(graph_lines);
        }

        lines.push(Line::raw(""));

        // Classification breakdown - combined with Legend
        lines.push(Line::from(Span::styled(
            "Move Quality",
            Style::default()
                .fg(self.theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )));

        let (white_counts, black_counts) = count_classifications(&review.positions);
        // Categories with symbols - show all even if count is 0 (legend purposes)
        let categories = [
            (
                "!! Brilliant",
                MoveClassification::ClassificationBrilliant as i32,
                self.theme.move_brilliant,
            ),
            (
                "!  Excellent",
                MoveClassification::ClassificationExcellent as i32,
                self.theme.move_excellent,
            ),
            (
                "   Good/Best",
                MoveClassification::ClassificationGood as i32,
                self.theme.move_good,
            ),
            (
                "?! Inaccuracy",
                MoveClassification::ClassificationInaccuracy as i32,
                self.theme.move_inaccuracy,
            ),
            (
                "?  Mistake",
                MoveClassification::ClassificationMistake as i32,
                self.theme.move_mistake,
            ),
            (
                "?? Blunder",
                MoveClassification::ClassificationBlunder as i32,
                self.theme.move_blunder,
            ),
            (
                "[] Forced",
                MoveClassification::ClassificationForced as i32,
                self.theme.move_forced,
            ),
        ];

        // Show all categories (combined legend + counts)
        for (label, class_val, color) in &categories {
            let w = white_counts.get(class_val).copied().unwrap_or(0);
            let b = black_counts.get(class_val).copied().unwrap_or(0);
            lines.push(Line::from(vec![
                Span::styled(format!("  {:14}", label), Style::default().fg(*color)),
                Span::raw(format!("W:{:<3} B:{}", w, b)),
            ]));
        }

        lines.push(Line::raw(""));

        // Critical moments
        let critical: Vec<_> = review
            .positions
            .iter()
            .filter(|p| {
                matches!(
                    MoveClassification::try_from(p.classification),
                    Ok(MoveClassification::ClassificationBlunder)
                        | Ok(MoveClassification::ClassificationMistake)
                )
            })
            .collect();

        if !critical.is_empty() {
            lines.push(Line::from(Span::styled(
                "Critical Moments",
                Style::default()
                    .fg(self.theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            )));

            for pos in critical.iter().take(10) {
                let is_white = is_white_ply(pos.ply);
                let move_num = pos.ply.div_ceil(2);
                let side = if is_white { "W" } else { "B" };
                let class_str = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => "??",
                    Ok(MoveClassification::ClassificationMistake) => "?",
                    _ => "",
                };
                let color = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => self.theme.move_blunder,
                    _ => self.theme.move_mistake,
                };

                lines.push(Line::from(vec![
                    Span::raw(format!("  {}. ", move_num)),
                    Span::raw(format!("[{}] ", side)),
                    Span::styled(
                        format!("{}{}", pos.played_san, class_str),
                        Style::default().fg(color),
                    ),
                    Span::raw(format!(" ({}cp)", pos.cp_loss)),
                ]));
            }
        }

        // Game-wide advanced analysis (psychology / momentum)
        if let Some(ref advanced) = self.review_state.advanced {
            let white_psy = advanced.white_psychology.as_ref();
            let black_psy = advanced.black_psychology.as_ref();

            // Phase performance
            lines.push(Line::from(Span::styled(
                "Phase Performance (avg cp_loss)",
                Style::default().fg(self.theme.info),
            )));

            let w_opening = white_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
            let b_opening = black_psy.map(|p| p.opening_avg_cp_loss).unwrap_or(0.0);
            let w_mid = white_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
            let b_mid = black_psy.map(|p| p.middlegame_avg_cp_loss).unwrap_or(0.0);
            let w_end = white_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);
            let b_end = black_psy.map(|p| p.endgame_avg_cp_loss).unwrap_or(0.0);

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Opening", Style::default().fg(self.theme.text_primary)),
                Span::raw(": W "),
                Span::styled(
                    format!("{:.1}", w_opening),
                    Style::default().fg(self.theme.info_light),
                ),
                Span::raw("  B "),
                Span::styled(
                    format!("{:.1}", b_opening),
                    Style::default().fg(self.theme.info_light),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Middlegame", Style::default().fg(self.theme.text_primary)),
                Span::raw(": W "),
                Span::styled(
                    format!("{:.1}", w_mid),
                    Style::default().fg(self.theme.info_light),
                ),
                Span::raw("  B "),
                Span::styled(
                    format!("{:.1}", b_mid),
                    Style::default().fg(self.theme.info_light),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Endgame", Style::default().fg(self.theme.text_primary)),
                Span::raw(": W "),
                Span::styled(
                    format!("{:.1}", w_end),
                    Style::default().fg(self.theme.info_light),
                ),
                Span::raw("  B "),
                Span::styled(
                    format!("{:.1}", b_end),
                    Style::default().fg(self.theme.info_light),
                ),
            ]));

            lines.push(Line::raw(""));

            // Error patterns
            let w_max_err = white_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
            let b_max_err = black_psy.map(|p| p.max_consecutive_errors).unwrap_or(0);
            let w_blunder = white_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);
            let b_blunder = black_psy.map(|p| p.blunder_cluster_density).unwrap_or(0);

            lines.push(Line::from(Span::styled(
                "Error Patterns",
                Style::default().fg(self.theme.info),
            )));
            lines.push(Line::from(vec![
                Span::raw("  Max consecutive: "),
                Span::styled(
                    format!("W:{}  B:{}", w_max_err, b_max_err),
                    Style::default().fg(self.theme.negative_light),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Blunder cluster: "),
                Span::styled(
                    format!("W:{}  B:{}", w_blunder, b_blunder),
                    Style::default().fg(self.theme.secondary_light),
                ),
            ]));

            lines.push(Line::raw(""));

            // Momentum
            let w_fav = white_psy.map(|p| p.favorable_swings).unwrap_or(0);
            let b_fav = black_psy.map(|p| p.favorable_swings).unwrap_or(0);
            let w_unfav = white_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
            let b_unfav = black_psy.map(|p| p.unfavorable_swings).unwrap_or(0);
            let w_streak = white_psy.map(|p| p.max_momentum_streak).unwrap_or(0);
            let b_streak = black_psy.map(|p| p.max_momentum_streak).unwrap_or(0);

            lines.push(Line::from(Span::styled(
                "Momentum",
                Style::default().fg(self.theme.info),
            )));
            lines.push(Line::from(vec![
                Span::raw("  Favorable swings: "),
                Span::styled(
                    format!("W:{}  B:{}", w_fav, b_fav),
                    Style::default().fg(self.theme.positive),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Unfavorable swings: "),
                Span::styled(
                    format!("W:{}  B:{}", w_unfav, b_unfav),
                    Style::default().fg(self.theme.negative),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  Max streak: "),
                Span::styled(
                    format!("W:{}  B:{}", w_streak, b_streak),
                    Style::default().fg(self.theme.warning),
                ),
            ]));

            lines.push(Line::raw(""));

            // Critical positions count
            lines.push(Line::from(vec![
                Span::styled(
                    "Critical positions: ",
                    Style::default().fg(self.theme.muted),
                ),
                Span::raw(format!("{}", advanced.critical_positions_count)),
            ]));

            lines.push(Line::raw(""));
        }

        // Analysis info
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Depth: ", Style::default().fg(self.theme.muted)),
            Span::raw(format!("{}", review.analysis_depth)),
            Span::raw("  "),
            Span::styled("Plies: ", Style::default().fg(self.theme.muted)),
            Span::raw(format!("{}/{}", review.analyzed_plies, review.total_plies)),
        ]));

        let content_height = lines.len() as u16;
        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(area, buf);

        if content_height > area.height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(self.theme.info).bg(self.theme.muted));
            let mut scrollbar_state =
                ScrollbarState::new(content_height as usize).position(self.scroll as usize);
            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}
