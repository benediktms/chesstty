use crate::app::FenHistory;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub enum FenDialogFocus {
    Input,
    HistoryList,
}

pub struct FenDialogState {
    pub input_buffer: String,
    pub focus: FenDialogFocus,
    pub selected_history_index: usize,
    pub validation_error: Option<String>,
}

impl FenDialogState {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            focus: FenDialogFocus::Input,
            selected_history_index: 0,
            validation_error: None,
        }
    }
}

impl Default for FenDialogState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FenDialogWidget<'a> {
    pub dialog_state: &'a FenDialogState,
    pub fen_history: &'a FenHistory,
}

impl<'a> FenDialogWidget<'a> {
    pub fn new(dialog_state: &'a FenDialogState, fen_history: &'a FenHistory) -> Self {
        Self {
            dialog_state,
            fen_history,
        }
    }
}

impl Widget for FenDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the background
        Clear.render(area, buf);

        // Calculate centered dialog area
        let dialog_width = 70;
        let dialog_height = 28;
        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x: area.x + x,
            y: area.y + y,
            width: dialog_width.min(area.width),
            height: dialog_height.min(area.height),
        };

        let block = Block::default()
            .title("♟ Enter FEN Position ♟")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Split into sections: Input (5 lines), History (remaining), Help (2 lines)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // Input section
                Constraint::Min(10),    // History list
                Constraint::Length(3),  // Help text + error
            ])
            .split(inner);

        // === Input Section ===
        let input_focused = self.dialog_state.focus == FenDialogFocus::Input;
        let input_border_style = if input_focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let input_text = if self.dialog_state.input_buffer.is_empty() {
            Span::styled(
                "Type or paste FEN string...",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled(&self.dialog_state.input_buffer, Style::default().fg(Color::White))
        };

        let input_widget = Paragraph::new(Line::from(vec![input_text]))
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .title(" Input FEN ")
                    .borders(Borders::ALL)
                    .border_style(input_border_style),
            );
        input_widget.render(chunks[0], buf);

        // === History List Section ===
        let list_focused = self.dialog_state.focus == FenDialogFocus::HistoryList;
        let list_border_style = if list_focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut lines = Vec::new();
        let entries = self.fen_history.entries();

        for (idx, entry) in entries.iter().enumerate() {
            let is_selected = idx == self.dialog_state.selected_history_index && list_focused;
            let prefix = if is_selected { "► " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Truncate FEN if too long
            let display_fen = if entry.fen.len() > 60 {
                format!("{}...", &entry.fen[..57])
            } else {
                entry.fen.clone()
            };

            let line = if let Some(label) = &entry.label {
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(label, style.fg(Color::Cyan)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(display_fen, style),
                ])
            };

            lines.push(line);
        }

        let history_widget = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .title(" Recent FENs ")
                    .borders(Borders::ALL)
                    .border_style(list_border_style),
            );
        history_widget.render(chunks[1], buf);

        // === Help Text + Error ===
        let mut help_lines = vec![Line::from(vec![Span::styled(
            "Tab/h/l: Switch Focus  ↑/↓/j/k: Navigate  Enter: Confirm  Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )])];

        if let Some(error) = &self.dialog_state.validation_error {
            help_lines.push(Line::from(vec![Span::styled(
                format!("Error: {}", error),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
        }

        let help_widget = Paragraph::new(help_lines).alignment(Alignment::Center);
        help_widget.render(chunks[2], buf);
    }
}
