use crate::ui::widgets::selectable_table::SelectableTableState;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, StatefulWidget, Table, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub enum FenDialogFocus {
    Input,
    PositionList,
}

pub struct FenDialogState {
    pub input_buffer: String,
    pub name_buffer: String,
    pub focus: FenDialogFocus,
    pub position_table: SelectableTableState,
    pub validation_error: Option<String>,
}

impl FenDialogState {
    pub fn new(position_count: usize) -> Self {
        Self {
            input_buffer: String::new(),
            name_buffer: String::new(),
            focus: FenDialogFocus::Input,
            position_table: SelectableTableState::new(position_count),
            validation_error: None,
        }
    }
}

pub struct FenDialogWidget<'a> {
    pub dialog_state: &'a mut FenDialogState,
    pub positions: &'a [chess_client::SavedPosition],
}

impl<'a> FenDialogWidget<'a> {
    pub fn new(
        dialog_state: &'a mut FenDialogState,
        positions: &'a [chess_client::SavedPosition],
    ) -> Self {
        Self {
            dialog_state,
            positions,
        }
    }
}

impl Widget for FenDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let dialog_width = 76;
        let dialog_height = 30;
        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x: area.x + x,
            y: area.y + y,
            width: dialog_width.min(area.width),
            height: dialog_height.min(area.height),
        };

        let block = Block::default()
            .title(" \u{265f} Select or Enter Position \u{265f} ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // Input section
                Constraint::Min(10),   // Positions table
                Constraint::Length(3), // Help text
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
                "Type FEN string, then Enter to save & use...",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled(
                self.dialog_state.input_buffer.clone(),
                Style::default().fg(Color::White),
            )
        };

        let input_widget = Paragraph::new(Line::from(vec![input_text]))
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .title(" FEN Input ")
                    .borders(Borders::ALL)
                    .border_style(input_border_style),
            );
        input_widget.render(chunks[0], buf);

        // === Positions Table ===
        let list_focused = self.dialog_state.focus == FenDialogFocus::PositionList;
        let list_border_style = if list_focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let header = Row::new(vec![
            Cell::from(Text::from("Name")).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from(Text::from("FEN")).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
        .height(1);

        let rows: Vec<Row> = self
            .positions
            .iter()
            .map(|pos| {
                let name_display = if pos.is_default {
                    format!("[D] {}", pos.name)
                } else {
                    pos.name.clone()
                };
                let fen_preview = if pos.fen.len() > 40 {
                    format!("{}...", &pos.fen[..37])
                } else {
                    pos.fen.clone()
                };
                Row::new(vec![
                    Cell::from(Text::from(name_display)),
                    Cell::from(Text::from(fen_preview)),
                ])
            })
            .collect();

        let highlight_style = if list_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let table = Table::new(rows, [Constraint::Length(28), Constraint::Min(30)])
            .header(header)
            .block(
                Block::default()
                    .title(" Saved Positions (Enter to select, d to delete) ")
                    .borders(Borders::ALL)
                    .border_style(list_border_style),
            )
            .highlight_style(highlight_style)
            .highlight_symbol(" \u{25b6} ");

        StatefulWidget::render(
            table,
            chunks[1],
            buf,
            &mut self.dialog_state.position_table.table_state,
        );

        // === Help Text ===
        let mut help_lines = vec![Line::from(vec![Span::styled(
            "Tab: Switch Focus  \u{2191}/\u{2193}/j/k: Navigate  Enter: Select/Save  d: Delete  Esc: Cancel",
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
