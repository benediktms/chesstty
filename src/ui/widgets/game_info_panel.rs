use crate::app::AppState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct GameInfoPanel<'a> {
    pub app_state: &'a AppState,
}

impl<'a> GameInfoPanel<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }
}

impl Widget for GameInfoPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("♟ Game Info ♟")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = vec![];

        // Game mode
        lines.push(Line::from(vec![
            ratatui::text::Span::styled(
                "Mode: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Span::raw(format_game_mode(&self.app_state.mode)),
        ]));

        lines.push(Line::raw(""));

        // Turn indicator
        let turn_color = self.app_state.game.side_to_move();
        let turn_text = match turn_color {
            cozy_chess::Color::White => "White to move",
            cozy_chess::Color::Black => "Black to move",
        };
        lines.push(Line::from(vec![
            ratatui::text::Span::styled(
                "Turn: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Span::styled(
                turn_text,
                Style::default()
                    .fg(if matches!(turn_color, cozy_chess::Color::White) {
                        Color::White
                    } else {
                        Color::Gray
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Add selection indicator
        if let Some(selected) = self.app_state.ui_state.selected_square {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    "Selected: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    format_square(selected),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            // List legal move destinations
            if !self.app_state.ui_state.highlighted_squares.is_empty() {
                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    "Legal: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));

                // Format squares as comma-separated list
                let moves_str: String = self
                    .app_state
                    .ui_state
                    .highlighted_squares
                    .iter()
                    .map(|&sq| format_square(sq))
                    .collect::<Vec<_>>()
                    .join(", ");

                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    moves_str,
                    Style::default().fg(Color::Green),
                )]));
            }
        }

        // Add status message
        if let Some(ref msg) = self.app_state.ui_state.status_message {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    "Status: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::raw(msg),
            ]));
        }

        // Add game status
        let status = self.app_state.game.status();
        if !matches!(status, cozy_chess::GameStatus::Ongoing) {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    "Game: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    format_game_status(status),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Add engine info if available
        if let Some(ref engine_info) = self.app_state.ui_state.engine_info {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![ratatui::text::Span::styled(
                "Engine: ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));

            if let Some(depth) = engine_info.depth {
                lines.push(Line::from(vec![
                    ratatui::text::Span::raw("  Depth: "),
                    ratatui::text::Span::styled(
                        depth.to_string(),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }

            if let Some(ref score) = engine_info.score {
                lines.push(Line::from(vec![
                    ratatui::text::Span::raw("  Eval: "),
                    ratatui::text::Span::styled(
                        format_score(score),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

fn format_game_mode(mode: &crate::app::GameMode) -> &str {
    match mode {
        crate::app::GameMode::HumanVsHuman => "Human vs Human",
        crate::app::GameMode::HumanVsEngine { .. } => "Human vs Engine",
        crate::app::GameMode::EngineVsEngine => "Engine vs Engine",
        crate::app::GameMode::AnalysisMode => "Analysis",
        crate::app::GameMode::ReviewMode => "Review",
    }
}

fn format_game_status(status: cozy_chess::GameStatus) -> String {
    match status {
        cozy_chess::GameStatus::Ongoing => "Ongoing".to_string(),
        cozy_chess::GameStatus::Won => "Checkmate!".to_string(),
        cozy_chess::GameStatus::Drawn => "Draw".to_string(),
    }
}

fn format_score(score: &crate::engine::Score) -> String {
    match score {
        crate::engine::Score::Centipawns(cp) => {
            let pawns = *cp as f32 / 100.0;
            format!("{:+.2}", pawns)
        }
        crate::engine::Score::Mate(moves) => {
            if *moves > 0 {
                format!("Mate in {}", moves)
            } else {
                format!("Mated in {}", moves.abs())
            }
        }
    }
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
