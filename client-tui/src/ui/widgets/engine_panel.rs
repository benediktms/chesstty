use crate::ui::theme::Theme;
use chess_client::EngineInfo;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct EngineAnalysisPanel<'a> {
    pub engine_info: Option<&'a EngineInfo>,
    pub is_thinking: bool,
    pub scroll: u16,
    pub theme: &'a Theme,
}

impl<'a> EngineAnalysisPanel<'a> {
    pub fn new(
        engine_info: Option<&'a EngineInfo>,
        is_thinking: bool,
        scroll: u16,
        theme: &'a Theme,
    ) -> Self {
        Self {
            engine_info,
            is_thinking,
            scroll,
            theme,
        }
    }
}

impl Widget for EngineAnalysisPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(info) = self.engine_info {
            let mut lines = Vec::new();

            // Depth and selective depth
            if let Some(depth) = info.depth {
                let seldepth_str = info
                    .seldepth
                    .map(|sd| format!("/{}", sd))
                    .unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled("Depth: ", Style::default().fg(self.theme.muted)),
                    Span::styled(
                        format!("{}{}", depth, seldepth_str),
                        Style::default()
                            .fg(self.theme.text_primary)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            // Score/Evaluation
            if let Some(ref score) = info.score {
                let (score_text, score_color) = parse_score(score, self.theme);
                lines.push(Line::from(vec![
                    Span::styled("Score: ", Style::default().fg(self.theme.muted)),
                    Span::styled(
                        score_text,
                        Style::default()
                            .fg(score_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            // Nodes and NPS
            if let Some(nodes) = info.nodes {
                let nps_str = info
                    .nps
                    .map(|n| format!(" ({}/s)", format_number(n)))
                    .unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled("Nodes: ", Style::default().fg(self.theme.muted)),
                    Span::styled(
                        format!("{}{}", format_number(nodes), nps_str),
                        Style::default().fg(self.theme.text_primary),
                    ),
                ]));
            }

            // Time
            if let Some(time_ms) = info.time_ms {
                lines.push(Line::from(vec![
                    Span::styled("Time: ", Style::default().fg(self.theme.muted)),
                    Span::styled(format_time(time_ms), Style::default().fg(self.theme.text_primary)),
                ]));
            }

            // Principal Variation
            if !info.pv.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Principal Variation:",
                    Style::default()
                        .fg(self.theme.warning)
                        .add_modifier(Modifier::BOLD),
                )));

                // Display PV moves (wrap if too long)
                let pv_text = info.pv.join(" ");
                let max_width = (area.width as usize).saturating_sub(2);

                for chunk in wrap_text(&pv_text, max_width) {
                    lines.push(Line::from(Span::styled(
                        chunk,
                        Style::default().fg(self.theme.info),
                    )));
                }
            }

            let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
            paragraph.render(area, buf);
        } else {
            // No engine info available
            let text = if self.is_thinking {
                "Engine is thinking..."
            } else {
                "No engine analysis available"
            };

            let paragraph = Paragraph::new(Line::from(Span::styled(
                text,
                Style::default()
                    .fg(self.theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )));
            paragraph.render(area, buf);
        }
    }
}

// Helper functions

fn parse_score(score: &str, theme: &Theme) -> (String, Color) {
    // Score format: "cp 25" (centipawns) or "mate 5" (mate in 5)
    let parts: Vec<&str> = score.split_whitespace().collect();
    if parts.len() < 2 {
        return (score.to_string(), theme.text_primary);
    }

    match parts[0] {
        "cp" => {
            // Centipawns - convert to pawns
            if let Ok(cp) = parts[1].parse::<i32>() {
                let pawns = cp as f32 / 100.0;
                let color = if pawns > 0.0 {
                    theme.eval_positive
                } else if pawns < 0.0 {
                    theme.eval_negative
                } else {
                    theme.eval_equal
                };
                (format!("{:+.2}", pawns), color)
            } else {
                (score.to_string(), theme.text_primary)
            }
        }
        "mate" => {
            // Mate in X moves
            if let Ok(moves) = parts[1].parse::<i32>() {
                let color = if moves > 0 {
                    theme.eval_mate_positive
                } else {
                    theme.eval_mate_negative
                };
                let sign = if moves > 0 { "+" } else { "" };
                (format!("{}M{}", sign, moves.abs()), color)
            } else {
                (score.to_string(), theme.text_primary)
            }
        }
        _ => (score.to_string(), theme.text_primary),
    }
}

fn format_number(n: u64) -> String {
    // Format large numbers with thousands separators
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for c in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(c);
        count += 1;
    }

    result.chars().rev().collect()
}

fn format_time(ms: u64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let secs = seconds % 60;
    let millis = ms % 1000;

    if minutes > 0 {
        format!("{}:{:02}.{:03}s", minutes, secs, millis)
    } else {
        format!("{}.{:03}s", secs, millis)
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
