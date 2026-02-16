use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Cell, Clear, Row, StatefulWidget, Table, TableState, Widget},
};

/// State for a selectable table. Wraps ratatui's TableState with bounds checking.
pub struct SelectableTableState {
    pub table_state: TableState,
    row_count: usize,
}

impl SelectableTableState {
    pub fn new(row_count: usize) -> Self {
        let mut table_state = TableState::default();
        if row_count > 0 {
            table_state.select(Some(0));
        }
        Self {
            table_state,
            row_count,
        }
    }

    pub fn move_up(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if selected > 0 {
                self.table_state.select(Some(selected - 1));
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if selected < self.row_count.saturating_sub(1) {
                self.table_state.select(Some(selected + 1));
            }
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.table_state.selected()
    }

    pub fn update_row_count(&mut self, new_count: usize) {
        self.row_count = new_count;
        if new_count == 0 {
            self.table_state.select(None);
        } else if let Some(sel) = self.table_state.selected() {
            if sel >= new_count {
                self.table_state.select(Some(new_count - 1));
            }
        } else {
            self.table_state.select(Some(0));
        }
    }
}

/// Renders a selectable table as a centered overlay dialog.
pub fn render_table_overlay(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
    column_widths: &[Constraint],
    state: &mut SelectableTableState,
    width: u16,
    height: u16,
) {
    let popup_area = centered_rect(width, height, area);

    // Clear background
    Clear.render(popup_area, buf);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let header_cells: Vec<Cell> = headers
        .iter()
        .map(|h| {
            Cell::from(Text::from(*h)).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let header = Row::new(header_cells).height(1);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|row_data| {
            let cells: Vec<Cell> = row_data
                .iter()
                .map(|cell| Cell::from(Text::from(cell.clone())))
                .collect();
            Row::new(cells).height(1)
        })
        .collect();

    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
        .bg(Color::DarkGray);

    let table = Table::new(table_rows, column_widths)
        .header(header)
        .highlight_style(highlight_style)
        .highlight_symbol(" \u{25b6} ");

    StatefulWidget::render(table, inner, buf, &mut state.table_state);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);

    horizontal[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty_no_selection() {
        let state = SelectableTableState::new(0);
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn test_new_nonempty_selects_first() {
        let state = SelectableTableState::new(5);
        assert_eq!(state.selected_index(), Some(0));
    }

    #[test]
    fn test_move_up_clamps_at_zero() {
        let mut state = SelectableTableState::new(5);
        state.move_up(); // Already at 0
        assert_eq!(state.selected_index(), Some(0));
    }

    #[test]
    fn test_move_down_clamps_at_max() {
        let mut state = SelectableTableState::new(3);
        state.move_down(); // 0 -> 1
        state.move_down(); // 1 -> 2
        state.move_down(); // 2 -> 2 (clamped)
        assert_eq!(state.selected_index(), Some(2));
    }

    #[test]
    fn test_move_down_then_up() {
        let mut state = SelectableTableState::new(5);
        state.move_down(); // 0 -> 1
        state.move_down(); // 1 -> 2
        state.move_up(); // 2 -> 1
        assert_eq!(state.selected_index(), Some(1));
    }

    #[test]
    fn test_update_row_count_shrinks() {
        let mut state = SelectableTableState::new(5);
        state.move_down(); // 0 -> 1
        state.move_down(); // 1 -> 2
        state.move_down(); // 2 -> 3
        state.update_row_count(2); // Clamp to 1
        assert_eq!(state.selected_index(), Some(1));
    }

    #[test]
    fn test_update_row_count_to_zero() {
        let mut state = SelectableTableState::new(3);
        state.update_row_count(0);
        assert_eq!(state.selected_index(), None);
    }
}
