use crate::review_state::ReviewState;
use crate::ui::widgets::review_helpers::{
    accuracy_bar, accuracy_color, build_eval_graph, count_classifications,
};
use chess::is_white_ply;
use chess_client::MoveClassification;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct ReviewSummaryPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
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
                "White" => Color::White,
                "Black" => Color::Gray,
                "Draw" => Color::Yellow,
                _ => Color::Red,
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
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(white_acc) = review.white_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  White: "),
                Span::styled(
                    format!("{:.1}%", white_acc),
                    Style::default().fg(accuracy_color(white_acc)),
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
                    Style::default().fg(accuracy_color(black_acc)),
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
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            let graph_width = (area.width as usize).saturating_sub(4).min(60);
            let graph_lines = build_eval_graph(&review.positions, graph_width);
            lines.extend(graph_lines);
        }

        lines.push(Line::raw(""));

        // Classification breakdown - combined with Legend
        lines.push(Line::from(Span::styled(
            "Move Quality",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        let (white_counts, black_counts) = count_classifications(&review.positions);
        // Categories with symbols - show all even if count is 0 (legend purposes)
        let categories = [
            (
                "!! Brilliant",
                MoveClassification::ClassificationBrilliant as i32,
                Color::Cyan,
            ),
            (
                "!  Excellent",
                MoveClassification::ClassificationExcellent as i32,
                Color::Cyan,
            ),
            (
                "   Good/Best",
                MoveClassification::ClassificationGood as i32,
                Color::White,
            ),
            (
                "?! Inaccuracy",
                MoveClassification::ClassificationInaccuracy as i32,
                Color::Yellow,
            ),
            (
                "?  Mistake",
                MoveClassification::ClassificationMistake as i32,
                Color::Magenta,
            ),
            (
                "?? Blunder",
                MoveClassification::ClassificationBlunder as i32,
                Color::Red,
            ),
            (
                "[] Forced",
                MoveClassification::ClassificationForced as i32,
                Color::DarkGray,
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
                    .fg(Color::White)
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
                    Ok(MoveClassification::ClassificationBlunder) => Color::Red,
                    _ => Color::Magenta,
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

        // Analysis info
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Depth: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", review.analysis_depth)),
            Span::raw("  "),
            Span::styled("Plies: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}/{}", review.analyzed_plies, review.total_plies)),
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

