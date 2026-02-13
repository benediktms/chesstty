use crate::app::{AppState, UciDirection};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct UciDebugPanel<'a> {
    pub app_state: &'a AppState,
}

impl<'a> UciDebugPanel<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }
}

impl Widget for UciDebugPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("🔧 UCI Debug Panel (Press @ to toggle) 🔧")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.app_state.ui_state.uci_log.is_empty() {
            let paragraph = Paragraph::new("No UCI messages yet. Start a game vs engine!");
            paragraph.render(inner, buf);
            return;
        }

        let mut lines = vec![];

        // Show most recent messages (reverse order, most recent at top)
        let visible_count = (inner.height as usize).saturating_sub(2);
        let start_idx = self
            .app_state
            .ui_state
            .uci_log
            .len()
            .saturating_sub(visible_count);

        for entry in self.app_state.ui_state.uci_log.iter().skip(start_idx) {
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

            let mut spans = vec![ratatui::text::Span::styled(
                prefix,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )];

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
                spans.push(ratatui::text::Span::styled(text, style));
            }

            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines);
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
