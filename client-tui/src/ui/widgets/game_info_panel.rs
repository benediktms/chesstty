use crate::state::ClientState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct GameInfoPanel<'a> {
    pub client_state: &'a ClientState,
}

impl<'a> GameInfoPanel<'a> {
    pub fn new(client_state: &'a ClientState) -> Self {
        Self { client_state }
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
            ratatui::text::Span::raw(format_game_mode(&self.client_state.mode)),
        ]));

        lines.push(Line::raw(""));

        // Input phase
        let phase_text = match self.client_state.ui.input_phase {
            crate::state::InputPhase::SelectPiece => "Select Piece",
            crate::state::InputPhase::SelectDestination => "Select Destination",
            crate::state::InputPhase::SelectPromotion { .. } => "Select Promotion (q/r/b/n)",
        };
        let phase_color = match self.client_state.ui.input_phase {
            crate::state::InputPhase::SelectPiece => Color::Green,
            crate::state::InputPhase::SelectDestination => Color::Cyan,
            crate::state::InputPhase::SelectPromotion { .. } => Color::Magenta,
        };
        lines.push(Line::from(vec![
            ratatui::text::Span::styled(
                "Phase: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Span::styled(
                phase_text,
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::raw(""));

        // Turn indicator
        let turn_str = self.client_state.side_to_move();
        let is_white_turn = turn_str == "white";
        let turn_text = if is_white_turn {
            "White to move"
        } else {
            "Black to move"
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
                    .fg(if is_white_turn {
                        Color::White
                    } else {
                        Color::Gray
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Timer display — read from server snapshot
        if let Some(ref timer) = self.client_state.snapshot.timer {
            let white_ms = timer.white_remaining_ms;
            let black_ms = timer.black_remaining_ms;
            let white_active = timer.active_side.as_deref() == Some("white");
            let black_active = timer.active_side.as_deref() == Some("black");

            let timer_color = |ms: u64, is_active: bool| -> Color {
                if ms < 10_000 {
                    Color::Red
                } else if ms < 60_000 {
                    Color::Yellow
                } else if is_active {
                    Color::Green
                } else {
                    Color::White
                }
            };

            let format_ms = |ms: u64| -> String {
                let secs = ms / 1000;
                let mins = secs / 60;
                let rem_secs = secs % 60;
                if secs < 10 {
                    let tenths = (ms % 1000) / 100;
                    format!("0:{:02}.{}", rem_secs, tenths)
                } else {
                    format!("{}:{:02}", mins, rem_secs)
                }
            };

            let white_indicator = if white_active { "\u{25b6} " } else { "  " };
            let black_indicator = if black_active { "\u{25b6} " } else { "  " };

            lines.push(Line::from(vec![
                ratatui::text::Span::styled(white_indicator, Style::default().fg(Color::White)),
                ratatui::text::Span::styled("\u{2654} ", Style::default().fg(Color::White)),
                ratatui::text::Span::styled(
                    format_ms(white_ms),
                    Style::default()
                        .fg(timer_color(white_ms, white_active))
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::raw("  "),
                ratatui::text::Span::styled(black_indicator, Style::default().fg(Color::Gray)),
                ratatui::text::Span::styled("\u{265a} ", Style::default().fg(Color::Gray)),
                ratatui::text::Span::styled(
                    format_ms(black_ms),
                    Style::default()
                        .fg(timer_color(black_ms, black_active))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Add selection indicator
        if let Some(selected) = self.client_state.ui.selected_square {
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
            if !self.client_state.ui.highlighted_squares.is_empty() {
                lines.push(Line::from(vec![ratatui::text::Span::styled(
                    "Legal: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));

                // Format squares as comma-separated list
                let moves_str: String = self
                    .client_state
                    .ui
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
        if let Some(ref msg) = self.client_state.ui.status_message {
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
        let status = self.client_state.status();
        if status != 0 {
            // 0 = Ongoing
            lines.push(Line::raw(""));
            let status_text = match status {
                1 => "Checkmate!",
                2 => "Stalemate",
                3 => "Draw",
                _ => "Unknown",
            };
            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    "Game: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    status_text,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

fn format_game_mode(mode: &crate::state::GameMode) -> &str {
    match mode {
        crate::state::GameMode::HumanVsHuman => "Human vs Human",
        crate::state::GameMode::HumanVsEngine { .. } => "Human vs Engine",
        crate::state::GameMode::EngineVsEngine => "Engine vs Engine",
        crate::state::GameMode::AnalysisMode => "Analysis",
        crate::state::GameMode::ReviewMode => "Review",
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
