use crate::state::{UciDirection, UciLogEntry};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct UciDebugPanel<'a> {
    pub uci_log: &'a [UciLogEntry],
    pub scroll: u16,
    pub is_selected: bool,
}

impl<'a> UciDebugPanel<'a> {
    pub fn new(uci_log: &'a [UciLogEntry], scroll: u16, is_selected: bool) -> Self {
        Self {
            uci_log,
            scroll,
            is_selected,
        }
    }
}

impl Widget for UciDebugPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.is_selected {
            "🔧 UCI Debug Panel [SELECTED] 🔧"
        } else {
            "🔧 UCI Debug Panel (@ to toggle) 🔧"
        };
        let border_style = if self.is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Magenta)
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if self.uci_log.is_empty() {
            let paragraph = Paragraph::new("No UCI messages yet. Start a game vs engine!");
            paragraph.render(inner, buf);
            return;
        }

        let mut lines = vec![];
        let max_width = (inner.width as usize).saturating_sub(2);

        // Show all messages and let scroll handle visibility
        for entry in self.uci_log.iter() {
            // Show move context if available
            if let Some(ref context) = entry.move_context {
                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    format!("─── {} ───", context),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]));
            }

            // Show direction indicator and message
            let (prefix, color) = match entry.direction {
                UciDirection::ToEngine => ("→ OUT: ", Color::Cyan),
                UciDirection::FromEngine => ("← IN:  ", Color::Green),
            };

            // Parse message for syntax highlighting
            let message_parts = parse_uci_message(&entry.message);

            // Build the full message with syntax highlighting
            let mut current_line_spans = vec![ratatui::text::Span::styled(
                prefix,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )];
            let mut current_line_length = prefix.len();

            for (text, highlight) in message_parts {
                let style = match highlight {
                    HighlightType::Command => Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    HighlightType::Value => Style::default().fg(Color::White),
                    HighlightType::Keyword => Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                    HighlightType::Normal => Style::default().fg(Color::Gray),
                };

                // Check if adding this text would exceed max_width
                if current_line_length + text.len() > max_width && !current_line_spans.is_empty() {
                    // Push current line and start a new one
                    lines.push(Line::from(current_line_spans));
                    current_line_spans = vec![ratatui::text::Span::styled(
                        "    ", // Indent wrapped lines
                        Style::default(),
                    )];
                    current_line_length = 4;
                }

                current_line_length += text.len();
                current_line_spans.push(ratatui::text::Span::styled(text, style));
            }

            // Push the last line
            if !current_line_spans.is_empty() {
                lines.push(Line::from(current_line_spans));
            }
        }

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);
    }
}

enum HighlightType {
    Command,
    Keyword,
    Value,
    Normal,
}

fn parse_uci_message(message: &str) -> Vec<(String, HighlightType)> {
    let mut parts = Vec::new();
    let tokens: Vec<&str> = message.split_whitespace().collect();

    if tokens.is_empty() {
        return parts;
    }

    // First token is usually the command
    parts.push((tokens[0].to_string() + " ", HighlightType::Command));

    let mut i = 1;
    while i < tokens.len() {
        let token = tokens[i];

        // Check if it's a keyword
        let highlight = match token {
            "position" | "go" | "stop" | "quit" | "uci" | "isready" | "ucinewgame" => {
                HighlightType::Command
            }
            "fen" | "moves" | "movetime" | "depth" | "infinite" | "name" | "value" => {
                HighlightType::Keyword
            }
            "info" | "score" | "cp" | "mate" | "pv" | "nodes" | "nps" | "time" => {
                HighlightType::Keyword
            }
            _ => {
                // Check if it's a number or move
                if token.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    HighlightType::Value
                } else if token.len() >= 4 && token.chars().take(2).all(|c| c.is_ascii_lowercase())
                {
                    HighlightType::Value // Likely a move like e2e4
                } else {
                    HighlightType::Normal
                }
            }
        };

        parts.push((token.to_string() + " ", highlight));
        i += 1;
    }

    parts
}
