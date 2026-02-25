use chess_client::{MoveClassification, MoveRecord, PositionReview};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::StatefulWidget,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

pub struct MoveHistoryPanel<'a> {
    pub history: &'a [MoveRecord],
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
    pub title: &'static str,
    pub number_key_hint: Option<char>,
    /// Optional review data for classification markers.
    pub review_positions: Option<&'a [PositionReview]>,
    /// When set (review mode), highlight the move at this 1-indexed ply.
    pub current_ply: Option<u32>,
}

impl<'a> MoveHistoryPanel<'a> {
    pub fn new(history: &'a [MoveRecord], scroll: u16, is_selected: bool) -> Self {
        Self {
            history,
            scroll,
            is_selected,
            expanded: false,
            title: "Move History",
            number_key_hint: None,
            review_positions: None,
            current_ply: None,
        }
    }

    pub fn with_title(mut self, title: &'static str, number_key_hint: Option<char>) -> Self {
        self.title = title;
        self.number_key_hint = number_key_hint;
        self
    }

    pub fn with_review_positions(mut self, positions: Option<&'a [PositionReview]>) -> Self {
        self.review_positions = positions;
        self
    }

    pub fn with_current_ply(mut self, ply: Option<u32>) -> Self {
        self.current_ply = ply;
        self
    }

    /// Calculate scroll position to keep current_ply visible.
    /// Centers the current ply in the visible area when possible.
    #[allow(dead_code)]
    pub fn calculate_scroll(&self, visible_height: u16) -> u16 {
        if let Some(current_ply) = self.current_ply {
            if current_ply == 0 {
                return 0;
            }
            let total_rows = self.history.len().div_ceil(2);
            if total_rows <= visible_height as usize {
                return 0;
            }
            let current_row = ((current_ply.saturating_sub(1)) / 2) as usize;
            let scroll_threshold = current_row.saturating_sub((visible_height / 2) as usize);
            scroll_threshold as u16
        } else {
            0
        }
    }
}

/// Format clock_ms as `[M:SS]` for display in the move history.
fn format_clock_span(positions: &[PositionReview], ply: usize) -> Option<String> {
    positions
        .iter()
        .find(|p| p.ply as usize == ply)
        .and_then(|p| p.clock_ms)
        .map(|ms| {
            let total_secs = ms / 1000;
            let m = total_secs / 60;
            let s = total_secs % 60;
            format!(" [{}:{:02}]", m, s)
        })
}

/// Returns a classification marker and color for a given ply's review data.
fn classification_marker(
    positions: &[PositionReview],
    ply: usize,
) -> Option<(&'static str, Color)> {
    positions.iter().find(|p| p.ply as usize == ply).and_then(
        |p| match MoveClassification::try_from(p.classification) {
            Ok(MoveClassification::ClassificationBrilliant) => Some(("!!", Color::Cyan)),
            Ok(MoveClassification::ClassificationExcellent) => Some(("!", Color::Cyan)),
            Ok(MoveClassification::ClassificationInaccuracy) => Some(("?!", Color::Yellow)),
            Ok(MoveClassification::ClassificationMistake) => Some(("?", Color::Magenta)),
            Ok(MoveClassification::ClassificationBlunder) => Some(("??", Color::Red)),
            Ok(MoveClassification::ClassificationForced) => Some(("[]", Color::DarkGray)),
            _ => None,
        },
    )
}

impl Widget for MoveHistoryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base_title = if self.is_selected {
            format!("{} [SELECTED]", self.title)
        } else {
            format!("[{}] {}", self.number_key_hint.unwrap_or(' '), self.title)
        };
        let title = if self.expanded {
            format!("{} (Expanded)", base_title)
        } else {
            base_title
        };
        let border_style = if self.is_selected || self.expanded {
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

        if self.history.is_empty() {
            let paragraph = Paragraph::new("No moves yet");
            paragraph.render(inner, buf);
            return;
        }

        let lines = if self.expanded {
            self.build_expanded_lines()
        } else {
            self.build_compact_lines()
        };

        let paragraph = Paragraph::new(lines).scroll((self.scroll, 0));
        paragraph.render(inner, buf);

        let total_rows = self.history.len().div_ceil(2);
        if total_rows > inner.height as usize {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
            let mut scrollbar_state =
                ScrollbarState::new(total_rows).position(self.scroll as usize);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

impl MoveHistoryPanel<'_> {
    fn build_compact_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![];

        for (i, record) in self.history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            let move_color = if is_white { Color::White } else { Color::Gray };

            let move_str = if !record.san.is_empty() {
                record.san.clone()
            } else {
                let capture =
                    if record.captured.is_some() && !record.captured.as_ref().unwrap().is_empty() {
                        "x"
                    } else {
                        ""
                    };
                format!("{}{}{}", record.from, capture, record.to)
            };

            // Highlight current ply in review mode
            let ply = (i as u32) + 1;
            let is_current = self.current_ply == Some(ply);
            let bg = if is_current {
                Color::DarkGray
            } else {
                Color::Reset
            };

            // Build move spans
            let mut move_spans = vec![Span::styled(
                move_str,
                Style::default()
                    .fg(move_color)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            )];

            // Add classification marker if review data is available
            if let Some(positions) = self.review_positions {
                if let Some((marker, color)) = classification_marker(positions, i + 1) {
                    move_spans.push(Span::styled(marker.to_string(), Style::default().fg(color)));
                }
                if let Some(clock_text) = format_clock_span(positions, i + 1) {
                    move_spans.push(Span::styled(
                        clock_text,
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            if is_white {
                let mut spans = vec![Span::styled(
                    format!("{}. ", move_number),
                    Style::default().fg(Color::Yellow),
                )];
                spans.extend(move_spans);
                lines.push(Line::from(spans));
            } else if let Some(last_line) = lines.last_mut() {
                last_line.spans.push(Span::raw("  "));
                last_line.spans.extend(move_spans);
            }
        }

        lines
    }

    fn build_expanded_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![];

        for (i, record) in self.history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            let move_color = if is_white { Color::White } else { Color::Gray };

            let ply = (i as u32) + 1;
            let is_current = self.current_ply == Some(ply);
            let bg = if is_current {
                Color::DarkGray
            } else {
                Color::Reset
            };

            let san = if !record.san.is_empty() {
                record.san.clone()
            } else {
                format!("{}-{}", record.from, record.to)
            };

            let description = describe_move(record, is_white);

            let prefix = if is_white {
                format!("{}. ", move_number)
            } else {
                "   ".to_string()
            };

            let mut spans = vec![
                ratatui::text::Span::styled(prefix, Style::default().fg(Color::Yellow).bg(bg)),
                ratatui::text::Span::styled(
                    format!("{:<8}", san),
                    Style::default()
                        .fg(move_color)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    description,
                    Style::default().fg(Color::DarkGray).bg(bg),
                ),
            ];

            if let Some(positions) = self.review_positions {
                if let Some(clock_text) = format_clock_span(positions, i + 1) {
                    spans.push(ratatui::text::Span::styled(
                        clock_text,
                        Style::default().fg(Color::DarkGray).bg(bg),
                    ));
                }
            }

            lines.push(Line::from(spans));
        }

        lines
    }
}

/// Generate a human-readable description of a move from a MoveRecord.
pub fn describe_move(record: &MoveRecord, is_white: bool) -> String {
    let color_name = if is_white { "White" } else { "Black" };
    let piece_name = piece_display_name(&record.piece);
    let from = &record.from;
    let to = &record.to;

    let captured = record.captured.as_ref().filter(|c| !c.is_empty());

    let promotion = record.promotion.as_ref().filter(|p| !p.is_empty());

    // Detect castling: king moving from e1/e8 to g1/g8 or c1/c8
    if record.piece == "K" {
        let is_kingside = (from == "e1" && to == "g1") || (from == "e8" && to == "g8");
        let is_queenside = (from == "e1" && to == "c1") || (from == "e8" && to == "c8");
        if is_kingside {
            return format!("{} castled kingside", color_name);
        }
        if is_queenside {
            return format!("{} castled queenside", color_name);
        }
    }

    // Detect en passant: pawn captures diagonally but captured piece info says pawn
    // and the from/to files differ (diagonal move)
    if record.piece == "P" {
        if let Some(captured_piece) = captured {
            let from_file = from.chars().next().unwrap_or(' ');
            let to_file = to.chars().next().unwrap_or(' ');
            if from_file != to_file && captured_piece == "P" {
                // Check if it's en passant: pawn captures pawn but lands on a different rank
                // than where the captured pawn was (the captured pawn was on the same rank as from)
                let from_rank = from.chars().nth(1).unwrap_or(' ');
                let to_rank = to.chars().nth(1).unwrap_or(' ');
                // En passant: white pawn on rank 5 captures to rank 6, or black on rank 4 to rank 3
                if (is_white && from_rank == '5' && to_rank == '6')
                    || (!is_white && from_rank == '4' && to_rank == '3')
                {
                    return format!("{} pawn captures en passant on {}", color_name, to);
                }
            }
        }
    }

    // Handle promotion with capture
    if let Some(promo) = promotion {
        let promo_name = promotion_piece_name(promo);
        if let Some(cap) = captured {
            let cap_name = piece_display_name(cap).to_lowercase();
            return format!(
                "{} pawn captures {} and promotes to {} on {}",
                color_name, cap_name, promo_name, to
            );
        }
        return format!("{} pawn promotes to {} on {}", color_name, promo_name, to);
    }

    // Handle regular capture
    if let Some(cap) = captured {
        let cap_name = piece_display_name(cap).to_lowercase();
        return format!(
            "{} {} on {} captures {} on {}",
            color_name, piece_name, from, cap_name, to
        );
    }

    // Regular move
    format!(
        "{} {} moved from {} to {}",
        color_name, piece_name, from, to
    )
}

fn piece_display_name(piece: &str) -> &str {
    match piece {
        "P" => "pawn",
        "N" => "knight",
        "B" => "bishop",
        "R" => "rook",
        "Q" => "queen",
        "K" => "king",
        _ => piece,
    }
}

fn promotion_piece_name(promo: &str) -> &str {
    match promo.to_lowercase().as_str() {
        "q" | "queen" => "queen",
        "r" | "rook" => "rook",
        "b" | "bishop" => "bishop",
        "n" | "knight" => "knight",
        _ => promo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(
        piece: &str,
        from: &str,
        to: &str,
        captured: Option<&str>,
        san: &str,
        promotion: Option<&str>,
    ) -> MoveRecord {
        MoveRecord {
            piece: piece.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            captured: captured.map(|s| s.to_string()),
            san: san.to_string(),
            fen_after: String::new(),
            promotion: promotion.map(|s| s.to_string()),
            clock_ms: None,
        }
    }

    #[test]
    fn test_describe_simple_pawn_move() {
        let record = make_record("P", "e2", "e4", None, "e4", None);
        assert_eq!(
            describe_move(&record, true),
            "White pawn moved from e2 to e4"
        );
    }

    #[test]
    fn test_describe_capture() {
        let record = make_record("B", "c4", "f7", Some("P"), "Bxf7+", None);
        assert_eq!(
            describe_move(&record, true),
            "White bishop on c4 captures pawn on f7"
        );
    }

    #[test]
    fn test_describe_kingside_castle() {
        let record = make_record("K", "e1", "g1", None, "O-O", None);
        assert_eq!(describe_move(&record, true), "White castled kingside");
    }

    #[test]
    fn test_describe_queenside_castle() {
        let record = make_record("K", "e8", "c8", None, "O-O-O", None);
        assert_eq!(describe_move(&record, false), "Black castled queenside");
    }

    #[test]
    fn test_describe_en_passant() {
        let record = make_record("P", "e5", "d6", Some("P"), "exd6", None);
        assert_eq!(
            describe_move(&record, true),
            "White pawn captures en passant on d6"
        );
    }

    #[test]
    fn test_describe_promotion() {
        let record = make_record("P", "e7", "e8", None, "e8=Q", Some("q"));
        assert_eq!(
            describe_move(&record, true),
            "White pawn promotes to queen on e8"
        );
    }

    #[test]
    fn test_describe_promotion_with_capture() {
        let record = make_record("P", "d7", "e8", Some("R"), "dxe8=Q", Some("q"));
        assert_eq!(
            describe_move(&record, true),
            "White pawn captures rook and promotes to queen on e8"
        );
    }

    #[test]
    fn test_describe_knight_move() {
        let record = make_record("N", "g8", "f6", None, "Nf6", None);
        assert_eq!(
            describe_move(&record, false),
            "Black knight moved from g8 to f6"
        );
    }

    fn make_position(ply: u32, classification: MoveClassification) -> PositionReview {
        PositionReview {
            ply,
            classification: classification as i32,
            ..Default::default()
        }
    }

    #[test]
    fn test_classification_marker_blunder() {
        let positions = vec![make_position(1, MoveClassification::ClassificationBlunder)];
        let (marker, color) = classification_marker(&positions, 1).unwrap();
        assert_eq!(marker, "??");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_classification_marker_mistake() {
        let positions = vec![make_position(2, MoveClassification::ClassificationMistake)];
        let (marker, color) = classification_marker(&positions, 2).unwrap();
        assert_eq!(marker, "?");
        assert_eq!(color, Color::Magenta);
    }

    #[test]
    fn test_classification_marker_best_returns_none() {
        let positions = vec![make_position(1, MoveClassification::ClassificationBest)];
        assert!(classification_marker(&positions, 1).is_none());
    }

    #[test]
    fn test_classification_marker_no_match_returns_none() {
        let positions = vec![make_position(5, MoveClassification::ClassificationBlunder)];
        assert!(classification_marker(&positions, 1).is_none());
    }

    #[test]
    fn test_with_review_positions_sets_field() {
        let history = vec![make_record("P", "e2", "e4", None, "e4", None)];
        let positions = vec![make_position(1, MoveClassification::ClassificationBlunder)];

        let panel = MoveHistoryPanel::new(&history, 0, false);
        assert!(panel.review_positions.is_none());

        let panel = panel.with_review_positions(Some(&positions));
        assert!(panel.review_positions.is_some());
        assert_eq!(panel.review_positions.unwrap().len(), 1);
    }

    #[test]
    fn test_compact_lines_include_classification_markers() {
        let history = vec![
            make_record("P", "e2", "e4", None, "e4", None),
            make_record("P", "e7", "e5", None, "e5", None),
        ];
        let positions = vec![
            make_position(1, MoveClassification::ClassificationBlunder), // ply 1: ??
            make_position(2, MoveClassification::ClassificationExcellent), // ply 2: !
        ];

        let panel =
            MoveHistoryPanel::new(&history, 0, false).with_review_positions(Some(&positions));
        let lines = panel.build_compact_lines();

        // Both moves on one line: "1. e4??  e5!"
        assert_eq!(lines.len(), 1);
        let spans: Vec<&str> = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            spans.contains(&"??"),
            "Expected blunder marker '??' in spans: {:?}",
            spans
        );
        assert!(
            spans.contains(&"!"),
            "Expected excellent marker '!' in spans: {:?}",
            spans
        );
    }

    #[test]
    fn test_compact_lines_without_review_omit_markers() {
        let history = vec![make_record("P", "e2", "e4", None, "e4", None)];

        let panel = MoveHistoryPanel::new(&history, 0, false);
        let lines = panel.build_compact_lines();

        assert_eq!(lines.len(), 1);
        // Should only have move number + SAN, no classification markers
        assert_eq!(lines[0].spans.len(), 2); // "1. " and "e4"
    }

    #[test]
    fn test_compact_lines_include_clock_spans() {
        let history = vec![
            make_record("P", "e2", "e4", None, "e4", None),
            make_record("P", "e7", "e5", None, "e5", None),
        ];
        let positions = vec![
            PositionReview {
                ply: 1,
                classification: MoveClassification::ClassificationBest as i32,
                clock_ms: Some(598_000), // 9:58
                ..Default::default()
            },
            PositionReview {
                ply: 2,
                classification: MoveClassification::ClassificationBlunder as i32,
                clock_ms: Some(585_000), // 9:45
                ..Default::default()
            },
        ];

        let panel =
            MoveHistoryPanel::new(&history, 0, false).with_review_positions(Some(&positions));
        let lines = panel.build_compact_lines();

        assert_eq!(lines.len(), 1);
        let spans: Vec<&str> = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            spans.contains(&" [9:58]"),
            "Expected clock span ' [9:58]' in spans: {:?}",
            spans
        );
        assert!(
            spans.contains(&" [9:45]"),
            "Expected clock span ' [9:45]' in spans: {:?}",
            spans
        );
    }

    #[test]
    fn test_compact_lines_no_clock_when_none() {
        let history = vec![make_record("P", "e2", "e4", None, "e4", None)];
        let positions = vec![PositionReview {
            ply: 1,
            classification: MoveClassification::ClassificationBest as i32,
            clock_ms: None,
            ..Default::default()
        }];

        let panel =
            MoveHistoryPanel::new(&history, 0, false).with_review_positions(Some(&positions));
        let lines = panel.build_compact_lines();

        // No clock span, no classification marker for Best — just move number + SAN
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 2); // "1. " and "e4"
    }
}
