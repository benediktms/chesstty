use chess_client::MoveRecord;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct MoveHistoryPanel<'a> {
    pub history: &'a [MoveRecord],
    pub scroll: u16,
    pub is_selected: bool,
    pub expanded: bool,
}

impl<'a> MoveHistoryPanel<'a> {
    pub fn new(history: &'a [MoveRecord], scroll: u16, is_selected: bool) -> Self {
        Self { history, scroll, is_selected, expanded: false }
    }

    pub fn expanded(history: &'a [MoveRecord], scroll: u16) -> Self {
        Self { history, scroll, is_selected: true, expanded: true }
    }
}

impl Widget for MoveHistoryPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.expanded {
            "\u{2654} Move History (Expanded) \u{2655}"
        } else if self.is_selected {
            "\u{2654} Move History \u{2655} [SELECTED]"
        } else {
            "\u{2654} Move History \u{2655}"
        };
        let border_style = if self.is_selected || self.expanded {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
    }
}

impl MoveHistoryPanel<'_> {
    fn build_compact_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![];

        for (i, record) in self.history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            let move_color = if is_white {
                Color::White
            } else {
                Color::Gray
            };

            let move_str = if !record.san.is_empty() {
                record.san.clone()
            } else {
                let capture = if record.captured.is_some() && !record.captured.as_ref().unwrap().is_empty() {
                    "x"
                } else {
                    ""
                };
                format!("{}{}{}", record.from, capture, record.to)
            };

            if is_white {
                lines.push(Line::from(vec![
                    ratatui::text::Span::styled(
                        format!("{}. ", move_number),
                        Style::default().fg(Color::Yellow),
                    ),
                    ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(move_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                if let Some(last_line) = lines.last_mut() {
                    last_line.spans.push(ratatui::text::Span::raw("  "));
                    last_line.spans.push(ratatui::text::Span::styled(
                        move_str,
                        Style::default()
                            .fg(move_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }

        lines
    }

    fn build_expanded_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![];

        for (i, record) in self.history.iter().enumerate() {
            let move_number = (i / 2) + 1;
            let is_white = i % 2 == 0;

            let move_color = if is_white {
                Color::White
            } else {
                Color::Gray
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

            lines.push(Line::from(vec![
                ratatui::text::Span::styled(
                    prefix,
                    Style::default().fg(Color::Yellow),
                ),
                ratatui::text::Span::styled(
                    format!("{:<8}", san),
                    Style::default()
                        .fg(move_color)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    description,
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
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

    let captured = record
        .captured
        .as_ref()
        .filter(|c| !c.is_empty());

    let promotion = record
        .promotion
        .as_ref()
        .filter(|p| !p.is_empty());

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
    if record.piece == "P" && captured.is_some() {
        let from_file = from.chars().next().unwrap_or(' ');
        let to_file = to.chars().next().unwrap_or(' ');
        if from_file != to_file {
            let captured_piece = captured.unwrap();
            if captured_piece == "P" {
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
        return format!(
            "{} pawn promotes to {} on {}",
            color_name, promo_name, to
        );
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
    format!("{} {} moved from {} to {}", color_name, piece_name, from, to)
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

    fn make_record(piece: &str, from: &str, to: &str, captured: Option<&str>, san: &str, promotion: Option<&str>) -> MoveRecord {
        MoveRecord {
            piece: piece.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            captured: captured.map(|s| s.to_string()),
            san: san.to_string(),
            fen_after: String::new(),
            promotion: promotion.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_describe_simple_pawn_move() {
        let record = make_record("P", "e2", "e4", None, "e4", None);
        assert_eq!(describe_move(&record, true), "White pawn moved from e2 to e4");
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
}
