use crate::app::AppState;
use cozy_chess::{Color as ChessColor, Piece, Square};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

pub struct PromotionWidget<'a> {
    pub app_state: &'a AppState,
    pub from: Square,
    pub to: Square,
}

impl<'a> PromotionWidget<'a> {
    pub fn new(app_state: &'a AppState, from: Square, to: Square) -> Self {
        Self {
            app_state,
            from,
            to,
        }
    }
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
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // Determine piece color (side that's promoting)
        let promoting_color = self.app_state.game.position().color_on(self.from);

        let pieces = [
            (Piece::Queen, 'Q', "Queen"),
            (Piece::Rook, 'R', "Rook"),
            (Piece::Bishop, 'B', "Bishop"),
            (Piece::Knight, 'N', "Knight"),
        ];

        let selected_piece = self.app_state.ui_state.selected_promotion_piece;

        let mut lines = vec![Line::raw("")];

        for (piece, key, name) in pieces.iter() {
            let is_selected = *piece == selected_piece;
            let prefix = if is_selected { "► " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Get piece symbol (white or black depending on promoting color)
            let piece_symbol = get_piece_symbol(*piece, promoting_color);

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{} ", piece_symbol), style),
                Span::styled(format!("{:<8}", name), style),
                Span::styled(format!("({})", key), Style::default().fg(Color::DarkGray)),
            ]);

            lines.push(line);
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled(
            "↑/↓/J/K or Q/R/B/N | Enter",
            Style::default().fg(Color::DarkGray),
        )]));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
        paragraph.render(inner, buf);
    }
}

fn get_piece_symbol(piece: Piece, color: Option<ChessColor>) -> char {
    match (piece, color) {
        (Piece::Queen, Some(ChessColor::White)) => '♕',
        (Piece::Queen, Some(ChessColor::Black)) => '♛',
        (Piece::Queen, None) => '♕',
        (Piece::Rook, Some(ChessColor::White)) => '♖',
        (Piece::Rook, Some(ChessColor::Black)) => '♜',
        (Piece::Rook, None) => '♖',
        (Piece::Bishop, Some(ChessColor::White)) => '♗',
        (Piece::Bishop, Some(ChessColor::Black)) => '♝',
        (Piece::Bishop, None) => '♗',
        (Piece::Knight, Some(ChessColor::White)) => '♘',
        (Piece::Knight, Some(ChessColor::Black)) => '♞',
        (Piece::Knight, None) => '♘',
        _ => '?',
    }
}
