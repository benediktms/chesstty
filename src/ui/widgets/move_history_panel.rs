use crate::app::AppState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct MoveHistoryPanel<'a> {
    pub app_state: &'a AppState,
}

impl<'a> MoveHistoryPanel<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }
}

impl Widget for MoveHistoryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("📜 Move History 📜")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        let history = self.app_state.game.history();

        if history.is_empty() {
            let paragraph = Paragraph::new("No moves yet");
            paragraph.render(inner, buf);
            return;
        }

        let mut lines = vec![];

        // Format moves in pairs (white, black)
        for (i, entry) in history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            if is_white {
                // Start a new line for white's move
                let move_str = format_move(entry.mv);
                lines.push(Line::from(vec![
                    ratatui::text::Span::styled(
                        format!("{}. ", move_number),
                        Style::default().fg(Color::Yellow),
                    ),
                    ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                // Add black's move to the same line
                let move_str = format_move(entry.mv);
                if let Some(last_line) = lines.last_mut() {
                    last_line.spans.push(ratatui::text::Span::raw("  "));
                    last_line.spans.push(ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(Color::Gray)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }

        // If the last move was white's and game is ongoing, show "...."
        if history.len() % 2 == 1
            && matches!(
                self.app_state.game.status(),
                cozy_chess::GameStatus::Ongoing
            )
        {
            if let Some(last_line) = lines.last_mut() {
                last_line.spans.push(ratatui::text::Span::raw("  "));
                last_line.spans.push(ratatui::text::Span::styled(
                    "....",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

fn format_move(mv: cozy_chess::Move) -> String {
    // Simple UCI format for now (e.g., "e2e4")
    // TODO: Implement proper SAN notation
    format!("{}{}", format_square(mv.from), format_square(mv.to))
}

fn format_square(sq: cozy_chess::Square) -> String {
    let file = match sq.file() {
        cozy_chess::File::A => 'a',
        cozy_chess::File::B => 'b',
        cozy_chess::File::C => 'c',
        cozy_chess::File::D => 'd',
        cozy_chess::File::E => 'e',
        cozy_chess::File::F => 'f',
        cozy_chess::File::G => 'g',
        cozy_chess::File::H => 'h',
    };
    let rank = match sq.rank() {
        cozy_chess::Rank::First => '1',
        cozy_chess::Rank::Second => '2',
        cozy_chess::Rank::Third => '3',
        cozy_chess::Rank::Fourth => '4',
        cozy_chess::Rank::Fifth => '5',
        cozy_chess::Rank::Sixth => '6',
        cozy_chess::Rank::Seventh => '7',
        cozy_chess::Rank::Eighth => '8',
    };
    format!("{}{}", file, rank)
}
