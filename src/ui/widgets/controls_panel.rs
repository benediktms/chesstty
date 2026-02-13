use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct ControlsPanel;

impl ControlsPanel {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for ControlsPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("⌨ Controls ⌨")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = vec![
            Line::from(vec![
                Span::styled("Move: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Type squares"),
            ]),
            Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::raw("(e.g., e2 then e4)"),
            ]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Esc ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("- Clear selection"),
            ]),
            Line::from(vec![
                Span::styled("u ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("- Undo last move"),
            ]),
            Line::from(vec![
                Span::styled("n ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("- New game"),
            ]),
            Line::from(vec![
                Span::styled("q ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("- Quit"),
            ]),
        ];

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}
