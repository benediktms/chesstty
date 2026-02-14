// Adapter widgets that bridge ClientState to the original widgets

use chess_proto::MoveRecord;
use cozy_chess::Square;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Board widget that renders a chess board
pub struct BoardWidget {
    pub fen: String,
    pub selected_square: Option<Square>,
    pub highlighted_squares: Vec<Square>,
    pub last_move: Option<(Square, Square)>,
}

impl Widget for BoardWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use crate::converters::format_square;

        // For now, render a simple FEN display
        // TODO: Integrate the full board rendering from the original widgets
        let block = Block::default()
            .title("Chess Board")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        // Simple ASCII board placeholder
        let mut lines = vec![
            Line::from("  a b c d e f g h"),
            Line::from("8 ♜ ♞ ♝ ♛ ♚ ♝ ♞ ♜"),
            Line::from("7 ♟ ♟ ♟ ♟ ♟ ♟ ♟ ♟"),
            Line::from("6 · · · · · · · ·"),
            Line::from("5 · · · · · · · ·"),
            Line::from("4 · · · · · · · ·"),
            Line::from("3 · · · · · · · ·"),
            Line::from("2 ♙ ♙ ♙ ♙ ♙ ♙ ♙ ♙"),
            Line::from("1 ♖ ♘ ♗ ♕ ♔ ♗ ♘ ♖"),
            Line::from(""),
            Line::from(vec![
                Span::styled("FEN: ", Style::default().fg(Color::Yellow)),
                Span::raw(&self.fen),
            ]),
        ];

        if let Some(selected) = self.selected_square {
            let mut status_line = vec![
                Span::styled("Selected: ", Style::default().fg(Color::Cyan)),
                Span::raw(format_square(selected)),
            ];

            if !self.highlighted_squares.is_empty() {
                status_line.push(Span::raw(" → "));
                let highlighted_str: Vec<String> = self.highlighted_squares
                    .iter()
                    .map(|sq| format_square(*sq))
                    .collect();
                status_line.push(Span::styled(
                    highlighted_str.join(", "),
                    Style::default().fg(Color::Green),
                ));
            }

            lines.push(Line::from(status_line));
        }

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false });
        paragraph.render(inner, buf);
    }
}

/// Game info panel
pub struct GameInfoPanel {
    pub fen: String,
    pub side_to_move: String,
    pub status: i32,
    pub move_count: usize,
}

impl Widget for GameInfoPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("Game Info")
            .borders(Borders::ALL);

        let status_text = match self.status {
            0 => "Ongoing",
            1 => "Checkmate",
            2 => "Stalemate",
            3 => "Draw",
            _ => "Unknown",
        };

        let lines = vec![
            Line::from(vec![
                Span::styled("To Move: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    &self.side_to_move,
                    Style::default().fg(if self.side_to_move == "white" {
                        Color::White
                    } else {
                        Color::Gray
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                Span::raw(status_text),
            ]),
            Line::from(vec![
                Span::styled("Moves: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", self.move_count)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Controls:", Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from("• Enter square (e.g., 'e2')"),
            Line::from("• Then destination ('e4')"),
            Line::from("• 'undo' or 'u' - Undo"),
            Line::from("• 'reset' or 'r' - Reset"),
            Line::from("• Ctrl+C - Quit"),
        ];

        let inner = block.inner(area);
        block.render(area, buf);

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

/// Move history panel
pub struct MoveHistoryPanel {
    pub history: Vec<MoveRecord>,
}

impl Widget for MoveHistoryPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("Move History")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = Vec::new();

        for (i, record) in self.history.iter().enumerate() {
            let move_num = i / 2 + 1;
            let is_white = i % 2 == 0;

            if is_white {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}. ", move_num), Style::default().fg(Color::DarkGray)),
                    Span::styled(&record.san, Style::default().fg(Color::White)),
                ]));
            } else {
                // Append to previous line
                if let Some(last_line) = lines.last_mut() {
                    last_line.spans.push(Span::raw(" "));
                    last_line.spans.push(Span::styled(&record.san, Style::default().fg(Color::Gray)));
                }
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No moves yet",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false });
        paragraph.render(inner, buf);
    }
}
