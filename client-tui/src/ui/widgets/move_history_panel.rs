use chess_proto::MoveRecord;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct MoveHistoryPanel<'a> {
    pub history: &'a [MoveRecord],
    pub scroll: u16,
}

impl<'a> MoveHistoryPanel<'a> {
    pub fn new(history: &'a [MoveRecord], scroll: u16) -> Self {
        Self { history, scroll }
    }
}

impl Widget for MoveHistoryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("♔ Move History ♕ (PgUp/PgDn to scroll)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.history.is_empty() {
            let paragraph = Paragraph::new("No moves yet");
            paragraph.render(inner, buf);
            return;
        }

        let mut lines = vec![];

        // Format moves in pairs (white, black)
        for (i, record) in self.history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            let move_color = if is_white {
                Color::White
            } else {
                Color::Gray
            };

            let move_str = if !record.san.is_empty() {
                record.san.clone()
            } else {
                // Fallback to simple from-to notation
                let capture = if record.captured.is_some() && !record.captured.as_ref().unwrap().is_empty() {
                    "x"
                } else {
                    ""
                };
                format!("{}{}{}", record.from, capture, record.to)
            };

            if is_white {
                // Start a new line for white's move
                lines.push(Line::from(vec![
                    ratatui::text::Span::styled(
                        format!("{}. ", move_number),
                        Style::default().fg(Color::Yellow),
                    ),
                    ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(move_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                // Add black's move to the same line
                if let Some(last_line) = lines.last_mut() {
                    last_line.spans.push(ratatui::text::Span::raw("  "));
                    last_line.spans.push(ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(move_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);
    }
}
