use crate::review_state::ReviewState;
use chess::is_white_ply;
use chess_client::{review_score, MoveClassification, PositionReview};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct ReviewSummaryPanel<'a> {
    pub review_state: &'a ReviewState,
    pub scroll: u16,
}

impl Widget for ReviewSummaryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line<'static>> = vec![];

        let review = &self.review_state.review;

        // Game winner at the very top
        if let Some(ref winner) = review.winner {
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
            lines.push(Line::from(Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::raw(""));
        }

        // === CURRENT POSITION ANALYSIS (TOP) ===
        if self.review_state.current_ply > 0 {
            lines.push(Line::from(Span::styled(
                "Current Position",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            if let Some(pos) = self.review_state.current_position() {
                let is_white = is_white_ply(pos.ply);
                let move_num = pos.ply.div_ceil(2);
                let side = if is_white { "White" } else { "Black" };

                lines.push(Line::from(vec![
                    Span::raw("Move "),
                    Span::styled(
                        format!("{}. {}", move_num, side),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                if let Some((marker, color)) = classification_marker(&pos.classification) {
                    lines
                        .last_mut()
                        .unwrap()
                        .push_span(Span::styled(marker, Style::default().fg(color)));
                }

                if pos.cp_loss > 0 {
                    lines.push(Line::from(vec![
                        Span::raw("  cp_loss: "),
                        Span::styled(
                            format!("{}", pos.cp_loss),
                            Style::default().fg(cp_loss_color(pos.cp_loss)),
                        ),
                    ]));
                }
            }

            // Check if there's advanced position analysis
            if let Some(adv_pos) = self.review_state.advanced_position() {
                if adv_pos.is_critical {
                    lines.push(Line::from(Span::styled(
                        "  \u{26A0} CRITICAL POSITION \u{26A0}",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                }
            }

            lines.push(Line::raw(""));
        }

        // Accuracy section
        lines.push(Line::from(Span::styled(
            "Accuracy",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(white_acc) = review.white_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  White: "),
                Span::styled(
                    format!("{:.1}%", white_acc),
                    Style::default().fg(accuracy_color(white_acc)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(white_acc, 20)),
            ]));
        }
        if let Some(black_acc) = review.black_accuracy {
            lines.push(Line::from(vec![
                Span::raw("  Black: "),
                Span::styled(
                    format!("{:.1}%", black_acc),
                    Style::default().fg(accuracy_color(black_acc)),
                ),
                Span::raw("  "),
                Span::raw(accuracy_bar(black_acc, 20)),
            ]));
        }

        // Eval graph
        if !review.positions.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Evaluation",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            let graph_width = (area.width as usize).saturating_sub(4).min(60);
            let graph_lines = build_eval_graph(&review.positions, graph_width);
            lines.extend(graph_lines);
        }

        lines.push(Line::raw(""));

        // Classification breakdown - combined with Legend
        lines.push(Line::from(Span::styled(
            "Move Quality",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        let (white_counts, black_counts) = count_classifications(&review.positions);
        // Categories with symbols - show all even if count is 0 (legend purposes)
        let categories = [
            (
                "!! Brilliant",
                MoveClassification::ClassificationBrilliant as i32,
                Color::Cyan,
            ),
            (
                "!  Excellent",
                MoveClassification::ClassificationExcellent as i32,
                Color::Cyan,
            ),
            (
                "   Good/Best",
                MoveClassification::ClassificationGood as i32,
                Color::White,
            ),
            (
                "?! Inaccuracy",
                MoveClassification::ClassificationInaccuracy as i32,
                Color::Yellow,
            ),
            (
                "?  Mistake",
                MoveClassification::ClassificationMistake as i32,
                Color::Magenta,
            ),
            (
                "?? Blunder",
                MoveClassification::ClassificationBlunder as i32,
                Color::Red,
            ),
            (
                "[] Forced",
                MoveClassification::ClassificationForced as i32,
                Color::DarkGray,
            ),
        ];

        // Show all categories (combined legend + counts)
        for (label, class_val, color) in &categories {
            let w = white_counts.get(class_val).copied().unwrap_or(0);
            let b = black_counts.get(class_val).copied().unwrap_or(0);
            lines.push(Line::from(vec![
                Span::styled(format!("  {:14}", label), Style::default().fg(*color)),
                Span::raw(format!("W:{:<3} B:{}", w, b)),
            ]));
        }

        lines.push(Line::raw(""));

        // Critical moments
        let critical: Vec<_> = review
            .positions
            .iter()
            .filter(|p| {
                matches!(
                    MoveClassification::try_from(p.classification),
                    Ok(MoveClassification::ClassificationBlunder)
                        | Ok(MoveClassification::ClassificationMistake)
                )
            })
            .collect();

        if !critical.is_empty() {
            lines.push(Line::from(Span::styled(
                "Critical Moments",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            for pos in critical.iter().take(10) {
                let is_white = is_white_ply(pos.ply);
                let move_num = pos.ply.div_ceil(2);
                let side = if is_white { "W" } else { "B" };
                let class_str = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => "??",
                    Ok(MoveClassification::ClassificationMistake) => "?",
                    _ => "",
                };
                let color = match MoveClassification::try_from(pos.classification) {
                    Ok(MoveClassification::ClassificationBlunder) => Color::Red,
                    _ => Color::Magenta,
                };

                lines.push(Line::from(vec![
                    Span::raw(format!("  {}. ", move_num)),
                    Span::raw(format!("[{}] ", side)),
                    Span::styled(
                        format!("{}{}", pos.played_san, class_str),
                        Style::default().fg(color),
                    ),
                    Span::raw(format!(" ({}cp)", pos.cp_loss)),
                ]));
            }
        }

        // Analysis info
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Depth: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", review.analysis_depth)),
            Span::raw("  "),
            Span::styled("Plies: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}/{}", review.analyzed_plies, review.total_plies)),
        ]));

        let content_height = lines.len() as u16;
        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(area, buf);

        if content_height > area.height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
            let mut scrollbar_state =
                ScrollbarState::new(content_height as usize).position(self.scroll as usize);
            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

fn accuracy_color(accuracy: f64) -> Color {
    if accuracy >= 90.0 {
        Color::Green
    } else if accuracy >= 70.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn accuracy_bar(accuracy: f64, width: usize) -> String {
    let filled = ((accuracy / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

/// Extract centipawn value from a proto ReviewScore, clamped to [-500, 500].
fn score_to_cp_clamped(pos: &PositionReview) -> i32 {
    let cp = match pos.eval_before.as_ref().and_then(|s| s.score.as_ref()) {
        Some(review_score::Score::Centipawns(cp)) => *cp,
        Some(review_score::Score::Mate(m)) => {
            if *m > 0 {
                500
            } else {
                -500
            }
        }
        None => 0,
    };
    cp.clamp(-500, 500)
}

/// Build an ASCII eval graph as a series of Line spans.
/// Renders a sparkline chart: positive = white advantage (above midline),
/// negative = black advantage (below midline).
/// Uses 5 rows of height. The midline is row 2 (0-indexed).
fn build_eval_graph(positions: &[PositionReview], width: usize) -> Vec<Line<'static>> {
    if positions.is_empty() || width == 0 {
        return vec![];
    }

    let height = 5usize;
    let mid = height / 2; // row 2 = midline (0.0 eval)

    // Sample positions to fit the available width
    let total = positions.len();
    let cols: Vec<(i32, Option<Color>)> = (0..width)
        .map(|col| {
            let idx = col * total / width;
            let idx = idx.min(total - 1);
            let pos = &positions[idx];
            let cp = score_to_cp_clamped(pos);

            // Color critical moments
            let col_color = match MoveClassification::try_from(pos.classification) {
                Ok(MoveClassification::ClassificationBlunder) => Some(Color::Red),
                Ok(MoveClassification::ClassificationMistake) => Some(Color::Yellow),
                _ => None,
            };
            (cp, col_color)
        })
        .collect();

    // Block characters for bar rendering (bottom to top within a cell)
    let blocks = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    // Build rows top to bottom
    let mut rows: Vec<Line<'static>> = Vec::with_capacity(height);
    for row in 0..height {
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for &(cp, critical_color) in &cols {
            // Map cp [-500, 500] to a fill level [0.0, height*8] sub-cells
            // midline at mid*8 sub-cells from bottom
            let max_sub = (height * 8) as f64;
            let mid_sub = (mid * 8) as f64;
            // cp=500 -> full top, cp=-500 -> full bottom, cp=0 -> midline
            let fill_sub = mid_sub + (cp as f64 / 500.0) * mid_sub;
            let fill_sub = fill_sub.clamp(0.0, max_sub) as usize;

            // This row spans sub-cells from (height-1-row)*8 to (height-row)*8
            let row_bottom = (height - 1 - row) * 8;
            let row_top = row_bottom + 8;

            let block_char = if fill_sub >= row_top {
                '\u{2588}' // fully filled
            } else if fill_sub <= row_bottom {
                ' ' // empty
            } else {
                blocks[fill_sub - row_bottom]
            };

            let fg = if row <= mid {
                // Above or at midline: white's territory
                critical_color.unwrap_or(Color::White)
            } else {
                critical_color.unwrap_or(Color::Gray)
            };

            spans.push(Span::styled(
                block_char.to_string(),
                Style::default().fg(fg),
            ));
        }
        rows.push(Line::from(spans));
    }

    rows
}

fn count_classifications(
    positions: &[chess_client::PositionReview],
) -> (
    std::collections::HashMap<i32, u32>,
    std::collections::HashMap<i32, u32>,
) {
    let mut white = std::collections::HashMap::new();
    let mut black = std::collections::HashMap::new();
    for p in positions {
        let map = if is_white_ply(p.ply) {
            &mut white
        } else {
            &mut black
        };
        *map.entry(p.classification).or_insert(0) += 1;
    }
    (white, black)
}

fn classification_marker(classification: &i32) -> Option<(&'static str, Color)> {
    match MoveClassification::try_from(*classification) {
        Ok(MoveClassification::ClassificationBrilliant) => Some(("!!", Color::Cyan)),
        Ok(MoveClassification::ClassificationExcellent) => Some(("!", Color::Cyan)),
        Ok(MoveClassification::ClassificationInaccuracy) => Some(("?!", Color::Yellow)),
        Ok(MoveClassification::ClassificationMistake) => Some(("?", Color::Magenta)),
        Ok(MoveClassification::ClassificationBlunder) => Some(("??", Color::Red)),
        Ok(MoveClassification::ClassificationForced) => Some(("[]", Color::DarkGray)),
        _ => None,
    }
}

fn cp_loss_color(cp_loss: i32) -> Color {
    if cp_loss < 10 {
        Color::Green
    } else if cp_loss < 30 {
        Color::Yellow
    } else if cp_loss < 60 {
        Color::Magenta
    } else {
        Color::Red
    }
}
