use crate::state::GameSession;
use crate::ui::fsm::UiStateMachine;
use chess_client::{review_score, MoveClassification, ReviewScore};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct GameInfoPanel<'a> {
    pub client_state: &'a GameSession,
    pub fsm: &'a UiStateMachine,
    pub is_selected: bool,
    pub scroll: u16,
}

impl<'a> GameInfoPanel<'a> {
    pub fn new(
        client_state: &'a GameSession,
        fsm: &'a UiStateMachine,
        is_selected: bool,
        scroll: u16,
    ) -> Self {
        Self {
            client_state,
            fsm,
            is_selected,
            scroll,
        }
    }
}

impl Widget for GameInfoPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.is_selected {
            "Game Info [SELECTED]"
        } else {
            "[1] Game Info"
        };
        let border_style = if self.is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = if self.client_state.review_state.is_some() {
            self.brender_stateld_review_lines()
        } else {
            self.brender_stateld_game_lines()
        };

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);
    }
}

impl GameInfoPanel<'_> {
    fn brender_stateld_review_lines(&self) -> Vec<Line<'static>> {
        use ratatui::text::Span;

        let mut lines = vec![];
        let label_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        // Mode
        lines.push(Line::from(vec![
            Span::styled("Mode: ", label_style),
            Span::styled(
                "Review",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        if let Some(ref rs) = self.client_state.review_state {
            // Ply X/Y + Turn
            let turn_str = self.client_state.side_to_move();
            let is_white_turn = turn_str == "white";
            let turn_text = if is_white_turn { "White" } else { "Black" };
            let turn_color = if is_white_turn {
                Color::White
            } else {
                Color::Gray
            };

            lines.push(Line::from(vec![
                Span::styled("Ply: ", label_style),
                Span::styled(
                    format!("{}/{}", rs.current_ply, rs.review.total_plies),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    turn_text.to_string(),
                    Style::default().fg(turn_color).add_modifier(Modifier::BOLD),
                ),
            ]));

            // Auto-play indicator
            if rs.auto_play {
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(
                    "AUTO-PLAY".to_string(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
            }

            // Add game status (winner/result) - from review data
            if let Some(ref rs) = self.client_state.review_state {
                if let Some(ref winner) = rs.review.winner {
                    lines.push(Line::raw(""));
                    let status_text = match winner.as_str() {
                        "White" => "White Wins!",
                        "Black" => "Black Wins!",
                        "Draw" => "Draw",
                        _ => "Unknown",
                    };
                    let status_color = match winner.as_str() {
                        "White" => Color::White,
                        "Black" => Color::Gray,
                        "Draw" => Color::Yellow,
                        _ => Color::Red,
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            "Result: ",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            status_text,
                            Style::default()
                                .fg(status_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            }
        }

        lines
    }

    fn brender_stateld_game_lines(&self) -> Vec<Line<'static>> {
        use ratatui::text::Span;

        let mut lines = vec![];

        // Game mode
        lines.push(Line::from(vec![
            Span::styled(
                "Mode: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format_game_mode(&self.client_state.mode)),
        ]));

        lines.push(Line::raw(""));

        // Input phase
        let phase_text = match self.fsm.input_phase {
            crate::ui::fsm::render_spec::InputPhase::SelectPiece => "Select Piece",
            crate::ui::fsm::render_spec::InputPhase::SelectDestination => "Select Destination",
            crate::ui::fsm::render_spec::InputPhase::SelectPromotion { .. } => {
                "Select Promotion (q/r/b/n)"
            }
        };
        let phase_color = match self.fsm.input_phase {
            crate::ui::fsm::render_spec::InputPhase::SelectPiece => Color::Green,
            crate::ui::fsm::render_spec::InputPhase::SelectDestination => Color::Cyan,
            crate::ui::fsm::render_spec::InputPhase::SelectPromotion { .. } => Color::Magenta,
        };
        lines.push(Line::from(vec![
            Span::styled(
                "Phase: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
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
            Span::styled(
                "Turn: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
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
                Span::styled(white_indicator, Style::default().fg(Color::White)),
                Span::styled("\u{2654} ", Style::default().fg(Color::White)),
                Span::styled(
                    format_ms(white_ms),
                    Style::default()
                        .fg(timer_color(white_ms, white_active))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(black_indicator, Style::default().fg(Color::Gray)),
                Span::styled("\u{265a} ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_ms(black_ms),
                    Style::default()
                        .fg(timer_color(black_ms, black_active))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Add selection indicator
        if let Some(selected) = self.client_state.selected_square {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "Selected: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format_square(selected),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            // List legal move destinations
            if !self.client_state.highlighted_squares.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "Legal: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));

                // Format squares as comma-separated list
                let moves_str: String = self
                    .client_state
                    .highlighted_squares
                    .iter()
                    .map(|&sq| format_square(sq))
                    .collect::<Vec<_>>()
                    .join(", ");

                lines.push(Line::from(vec![Span::styled(
                    moves_str,
                    Style::default().fg(Color::Green),
                )]));
            }
        }

        // Add status message
        if let Some(ref msg) = self.client_state.status_message {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "Status: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(msg.clone()),
            ]));
        }

        // Add game status
        let status = self.client_state.status();
        if status != 0 {
            lines.push(Line::raw(""));
            let status_text = match status {
                1 => "Checkmate!",
                2 => "Stalemate",
                3 => "Draw",
                _ => "Unknown",
            };
            lines.push(Line::from(vec![
                Span::styled(
                    "Game: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    status_text,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));

            // Add winner display (derived from status and side to move)
            if status == 1 {
                let side_to_move = self.client_state.side_to_move();
                let winner = if side_to_move == "white" {
                    "Black"
                } else {
                    "White"
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        "Winner: ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        winner,
                        Style::default()
                            .fg(if winner == "White" {
                                Color::White
                            } else {
                                Color::Gray
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else if status == 2 || status == 3 {
                lines.push(Line::from(vec![
                    Span::styled(
                        "Result: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Draw",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }

        lines
    }
}

fn format_game_mode(mode: &crate::state::GameMode) -> &'static str {
    match mode {
        crate::state::GameMode::HumanVsHuman => "Human vs Human",
        crate::state::GameMode::HumanVsEngine { .. } => "Human vs Engine",
        crate::state::GameMode::EngineVsEngine => "Engine vs Engine",
        crate::state::GameMode::AnalysisMode => "Analysis",
        crate::state::GameMode::ReviewMode => "Review",
    }
}

/// Format a ReviewScore as a human-readable string with appropriate color.
pub(crate) fn format_review_score(score: &ReviewScore) -> (String, Color) {
    match score.score.as_ref() {
        Some(review_score::Score::Centipawns(cp)) => {
            let pawns = *cp as f32 / 100.0;
            let color = if pawns > 0.5 {
                Color::Green
            } else if pawns < -0.5 {
                Color::Red
            } else {
                Color::White
            };
            (format!("{:+.2}", pawns), color)
        }
        Some(review_score::Score::Mate(m)) => {
            let color = if *m > 0 {
                Color::LightGreen
            } else {
                Color::LightRed
            };
            let sign = if *m > 0 { "+" } else { "" };
            (format!("{}M{}", sign, m.abs()), color)
        }
        None => ("N/A".to_string(), Color::DarkGray),
    }
}

/// Get the color for a classification value.
pub(crate) fn classification_color(classification: i32) -> Color {
    match MoveClassification::try_from(classification) {
        Ok(MoveClassification::ClassificationBrilliant) => Color::Cyan,
        Ok(MoveClassification::ClassificationBest) => Color::LightGreen,
        Ok(MoveClassification::ClassificationExcellent) => Color::Cyan,
        Ok(MoveClassification::ClassificationGood) => Color::White,
        Ok(MoveClassification::ClassificationInaccuracy) => Color::Yellow,
        Ok(MoveClassification::ClassificationMistake) => Color::Magenta,
        Ok(MoveClassification::ClassificationBlunder) => Color::Red,
        Ok(MoveClassification::ClassificationForced) => Color::DarkGray,
        Ok(MoveClassification::ClassificationBook) => Color::DarkGray,
        _ => Color::White,
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
