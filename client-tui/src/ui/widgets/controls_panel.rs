use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct ControlsPanel<'a> {
    pub input_buffer: &'a str,
}

impl<'a> ControlsPanel<'a> {
    pub fn new(input_buffer: &'a str) -> Self {
        Self {
            input_buffer,
        }
    }
}

impl Widget for ControlsPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("⌨ Controls ⌨")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = vec![];

        // Show input buffer if not empty
        if !self.input_buffer.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Input: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("> {}", self.input_buffer), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::raw(""));
        }

        // Show key controls in a cleaner format
        lines.push(Line::from(vec![
            Span::styled("Game Controls", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::raw(""));

        lines.push(Line::from(vec![
            Span::styled("u ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Undo Move"),
        ]));

        lines.push(Line::from(vec![
            Span::styled("r ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Reset Game"),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Esc ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Clear Selection"),
        ]));

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("View Controls", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::raw(""));

        lines.push(Line::from(vec![
            Span::styled("@ ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle UCI Debug"),
        ]));

        lines.push(Line::from(vec![
            Span::styled("# ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle Engine Panel"),
        ]));

        lines.push(Line::from(vec![
            Span::styled("PgUp/PgDn ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Scroll Panels"),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Home/End ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Jump to Top/Bottom"),
        ]));

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Ctrl+C ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Quit"),
        ]));

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}
