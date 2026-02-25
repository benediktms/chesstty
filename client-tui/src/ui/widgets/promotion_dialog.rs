use crate::ui::theme::Theme;
use cozy_chess::Piece;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub struct PromotionWidget<'a> {
    pub selected_piece: Piece,
    pub theme: &'a Theme,
}

impl Widget for PromotionWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the background
        Clear.render(area, buf);

        // Calculate centered dialog area
        let dialog_width = 30;
        let dialog_height = 10;
        let x = (area.width.saturating_sub(dialog_width)) / 2;
        let y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x: area.x + x,
            y: area.y + y,
            width: dialog_width.min(area.width),
            height: dialog_height.min(area.height),
        };

        let block = Block::default()
            .title("♟ Select Promotion ♟")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.dialog_border))
            .style(Style::default().bg(self.theme.dialog_bg));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let pieces = [
            (Piece::Queen, 'q', "Queen", '♕'),
            (Piece::Rook, 'r', "Rook", '♖'),
            (Piece::Bishop, 'b', "Bishop", '♗'),
            (Piece::Knight, 'n', "Knight", '♘'),
        ];

        let mut lines = vec![Line::raw("")];

        for (piece, key, name, symbol) in pieces.iter() {
            let is_selected = *piece == self.selected_piece;
            let prefix = if is_selected { "► " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(self.theme.dialog_highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_primary)
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{} ", symbol), style),
                Span::styled(format!("{:<8}", name), style),
                Span::styled(
                    format!("({})", key),
                    Style::default().fg(self.theme.muted),
                ),
            ]);

            lines.push(line);
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled(
            "Press q/r/b/n to select | Esc to cancel",
            Style::default().fg(self.theme.muted),
        )]));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
        paragraph.render(inner, buf);
    }
}
