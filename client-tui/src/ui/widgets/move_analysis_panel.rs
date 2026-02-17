use crate::review_state::ReviewState;
use crate::ui::widgets::game_info_panel::{classification_color, format_review_score};
use chess_client::{review_score, MoveClassification};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct MoveAnalysisPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
}

impl<'a> MoveAnalysisPanel<'a> {
    pub fn new(review_state: &'a ReviewState, scroll: u16, is_selected: bool) -> Self {
        Self {
            review_state,
            scroll,
            is_selected,
            expanded: false,
        }
    }
}

impl Widget for MoveAnalysisPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.expanded {
            "Move Analysis (Expanded)"
        } else if self.is_selected {
            "Move Analysis [SELECTED]"
        } else {
            "Move Analysis"
        };

        let border_style = if self.is_selected || self.expanded {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        let Some(pos) = self.review_state.current_position() else {
            let text = "Navigate to a move to see analysis";
            let paragraph = Paragraph::new(Line::from(Span::styled(
                text,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
            paragraph.render(inner, buf);
            return;
        };

        let mut lines: Vec<Line<'static>> = vec![];

        // Classification badge (colored header)
        let class_name = classification_display_name(pos.classification);
        let class_color = classification_color(pos.classification);
        let badge_text = format!(" {} ", class_name);
        lines.push(Line::from(Span::styled(
            badge_text,
            Style::default()
                .fg(Color::Black)
                .bg(class_color)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));

        // Played move with marker
        let marker = classification_marker_str(pos.classification);
        lines.push(Line::from(vec![
            Span::styled("Played: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{}", pos.played_san, marker),
                Style::default()
                    .fg(class_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Best move
        if !pos.best_move_san.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Best:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    pos.best_move_san.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines.push(Line::raw(""));

        // Eval before
        if let Some(ref score) = pos.eval_before {
            let (text, color) = format_review_score(score);
            let mate_info = format_mate_info(score);
            lines.push(Line::from(vec![
                Span::styled("Before: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(mate_info),
            ]));
        }

        // Eval after
        if let Some(ref score) = pos.eval_after {
            let (text, color) = format_review_score(score);
            let mate_info = format_mate_info(score);
            lines.push(Line::from(vec![
                Span::styled("After:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(mate_info),
            ]));
        }

        // Eval best
        if let Some(ref score) = pos.eval_best {
            let (text, color) = format_review_score(score);
            lines.push(Line::from(vec![
                Span::styled("Best:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // CP Loss
        if pos.cp_loss > 0 {
            lines.push(Line::from(vec![
                Span::styled("Loss:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}cp", pos.cp_loss),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        // Depth
        if pos.depth > 0 {
            lines.push(Line::from(vec![
                Span::styled("Depth:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}", pos.depth), Style::default().fg(Color::White)),
            ]));
        }

        // PV line
        if !pos.pv.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Principal Variation:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));

            let pv_text = pos.pv.join(" ");
            let max_width = (inner.width as usize).saturating_sub(2);
            for chunk in wrap_text(&pv_text, max_width) {
                lines.push(Line::from(Span::styled(
                    chunk,
                    Style::default().fg(Color::Cyan),
                )));
            }
        }

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);
    }
}

fn classification_display_name(classification: i32) -> &'static str {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => "BRILLIANT",
        Ok(MoveClassification::ClassificationBest) => "BEST",
        Ok(MoveClassification::ClassificationExcellent) => "EXCELLENT",
        Ok(MoveClassification::ClassificationGood) => "GOOD",
        Ok(MoveClassification::ClassificationInaccuracy) => "INACCURACY",
        Ok(MoveClassification::ClassificationMistake) => "MISTAKE",
        Ok(MoveClassification::ClassificationBlunder) => "BLUNDER",
        Ok(MoveClassification::ClassificationForced) => "FORCED",
        Ok(MoveClassification::ClassificationBook) => "BOOK",
        _ => "UNKNOWN",
    }
}

fn classification_marker_str(classification: i32) -> &'static str {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => "!!",
        Ok(MoveClassification::ClassificationExcellent) => "!",
        Ok(MoveClassification::ClassificationInaccuracy) => "?!",
        Ok(MoveClassification::ClassificationMistake) => "?",
        Ok(MoveClassification::ClassificationBlunder) => "??",
        Ok(MoveClassification::ClassificationForced) => "[]",
        _ => "",
    }
}

fn format_mate_info(score: &chess_client::ReviewScore) -> String {
    match score.score.as_ref() {
        Some(review_score::Score::Mate(m)) => {
            format!("  (Mate in {})", m.abs())
        }
        _ => String::new(),
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
