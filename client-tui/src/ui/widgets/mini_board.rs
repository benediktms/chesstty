use cozy_chess::{Board, Color as ChessColor, File, Piece, Rank, Square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Maps a chess piece + color to a Unicode symbol.
pub fn piece_to_unicode(piece: Piece, color: ChessColor) -> &'static str {
    match (piece, color) {
        (Piece::King, ChessColor::White) => "\u{2654}",   // ♔
        (Piece::Queen, ChessColor::White) => "\u{2655}",  // ♕
        (Piece::Rook, ChessColor::White) => "\u{2656}",   // ♖
        (Piece::Bishop, ChessColor::White) => "\u{2657}", // ♗
        (Piece::Knight, ChessColor::White) => "\u{2658}", // ♘
        (Piece::Pawn, ChessColor::White) => "\u{2659}",   // ♙
        (Piece::King, ChessColor::Black) => "\u{265a}",   // ♚
        (Piece::Queen, ChessColor::Black) => "\u{265b}",  // ♛
        (Piece::Rook, ChessColor::Black) => "\u{265c}",   // ♜
        (Piece::Bishop, ChessColor::Black) => "\u{265d}", // ♝
        (Piece::Knight, ChessColor::Black) => "\u{265e}", // ♞
        (Piece::Pawn, ChessColor::Black) => "\u{265f}",   // ♟
    }
}

/// Symbol for an empty square.
pub fn empty_square_symbol() -> &'static str {
    "\u{00b7}" // ·
}

/// Compact board widget using Unicode chess symbols on a white background.
/// Size: ~18 wide x 10 tall (including file/rank labels and border).
pub struct MiniBoardWidget<'a> {
    pub board: &'a Board,
    pub flipped: bool,
}

impl Widget for MiniBoardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::White).fg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 17 || inner.height < 9 {
            return; // Not enough space
        }

        let ranks: Vec<Rank> = if self.flipped {
            vec![
                Rank::First,
                Rank::Second,
                Rank::Third,
                Rank::Fourth,
                Rank::Fifth,
                Rank::Sixth,
                Rank::Seventh,
                Rank::Eighth,
            ]
        } else {
            vec![
                Rank::Eighth,
                Rank::Seventh,
                Rank::Sixth,
                Rank::Fifth,
                Rank::Fourth,
                Rank::Third,
                Rank::Second,
                Rank::First,
            ]
        };

        let files: Vec<File> = if self.flipped {
            vec![
                File::H,
                File::G,
                File::F,
                File::E,
                File::D,
                File::C,
                File::B,
                File::A,
            ]
        } else {
            vec![
                File::A,
                File::B,
                File::C,
                File::D,
                File::E,
                File::F,
                File::G,
                File::H,
            ]
        };

        let mut lines = vec![];

        for rank in &ranks {
            let rank_label = match rank {
                Rank::First => "1",
                Rank::Second => "2",
                Rank::Third => "3",
                Rank::Fourth => "4",
                Rank::Fifth => "5",
                Rank::Sixth => "6",
                Rank::Seventh => "7",
                Rank::Eighth => "8",
            };

            let mut spans = vec![Span::styled(
                format!("{} ", rank_label),
                Style::default().fg(Color::DarkGray).bg(Color::White),
            )];

            for file in &files {
                let sq = Square::new(*file, *rank);

                let symbol = {
                    let mut found = None;
                    for &color in &[ChessColor::White, ChessColor::Black] {
                        for &piece in &[
                            Piece::King,
                            Piece::Queen,
                            Piece::Rook,
                            Piece::Bishop,
                            Piece::Knight,
                            Piece::Pawn,
                        ] {
                            let bitboard = self.board.colored_pieces(color, piece);
                            if bitboard.has(sq) {
                                found = Some((piece, color));
                            }
                        }
                    }
                    match found {
                        Some((piece, color)) => piece_to_unicode(piece, color),
                        None => empty_square_symbol(),
                    }
                };

                spans.push(Span::styled(
                    format!("{} ", symbol),
                    Style::default().fg(Color::Black).bg(Color::White),
                ));
            }

            lines.push(Line::from(spans));
        }

        // File labels
        let file_labels: Vec<&str> = if self.flipped {
            vec!["h", "g", "f", "e", "d", "c", "b", "a"]
        } else {
            vec!["a", "b", "c", "d", "e", "f", "g", "h"]
        };
        let mut file_spans = vec![Span::styled("  ", Style::default().bg(Color::White))];
        for label in &file_labels {
            file_spans.push(Span::styled(
                format!("{} ", label),
                Style::default().fg(Color::DarkGray).bg(Color::White),
            ));
        }
        lines.push(Line::from(file_spans));

        let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::White));
        paragraph.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_pieces_to_unicode() {
        assert_eq!(piece_to_unicode(Piece::King, ChessColor::White), "\u{2654}");
        assert_eq!(
            piece_to_unicode(Piece::Queen, ChessColor::White),
            "\u{2655}"
        );
        assert_eq!(piece_to_unicode(Piece::Rook, ChessColor::White), "\u{2656}");
        assert_eq!(
            piece_to_unicode(Piece::Bishop, ChessColor::White),
            "\u{2657}"
        );
        assert_eq!(
            piece_to_unicode(Piece::Knight, ChessColor::White),
            "\u{2658}"
        );
        assert_eq!(piece_to_unicode(Piece::Pawn, ChessColor::White), "\u{2659}");
    }

    #[test]
    fn test_black_pieces_to_unicode() {
        assert_eq!(piece_to_unicode(Piece::King, ChessColor::Black), "\u{265a}");
        assert_eq!(
            piece_to_unicode(Piece::Queen, ChessColor::Black),
            "\u{265b}"
        );
        assert_eq!(piece_to_unicode(Piece::Rook, ChessColor::Black), "\u{265c}");
        assert_eq!(
            piece_to_unicode(Piece::Bishop, ChessColor::Black),
            "\u{265d}"
        );
        assert_eq!(
            piece_to_unicode(Piece::Knight, ChessColor::Black),
            "\u{265e}"
        );
        assert_eq!(piece_to_unicode(Piece::Pawn, ChessColor::Black), "\u{265f}");
    }

    #[test]
    fn test_empty_square_symbol() {
        assert_eq!(empty_square_symbol(), "\u{00b7}");
    }
}
