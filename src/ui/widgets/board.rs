use crate::app::AppState;
use cozy_chess::{Color as ChessColor, File, Piece, Rank, Square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

const SQUARE_WIDTH: u16 = 6;
const SQUARE_HEIGHT: u16 = 3;

pub struct BoardWidget<'a> {
    pub app_state: &'a AppState,
}

impl<'a> BoardWidget<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }
}

impl Widget for BoardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("♟ Chess Board ♟")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        block.render(area, buf);

        // Draw rank labels (8-1) on the left
        for rank_idx in 0..8 {
            let y = inner.y + (rank_idx as u16 * SQUARE_HEIGHT) + 1;
            if y < inner.bottom() {
                let rank_label = format!("{} ", 8 - rank_idx);
                buf.set_string(
                    inner.x.saturating_sub(2),
                    y,
                    &rank_label,
                    Style::default().fg(Color::Yellow),
                );
            }
        }

        // Draw file labels (a-h) at the bottom
        for file_idx in 0..8 {
            let x = inner.x + (file_idx as u16 * SQUARE_WIDTH) + 2;
            let y = inner.y + (8 * SQUARE_HEIGHT); // Right after the last rank
            if x < area.right() && y < area.bottom() {
                let file_label = format!("{}", (b'a' + file_idx) as char);
                buf.set_string(
                    x,
                    y,
                    &file_label,
                    Style::default().fg(Color::Yellow),
                );
            }
        }

        // Draw each square
        for rank_idx in 0..8 {
            for file_idx in 0..8 {
                let file = File::index(file_idx);
                let rank = Rank::index(7 - rank_idx); // Top rank is 8
                let square = Square::new(file, rank);

                let x = inner.x + (file_idx as u16 * SQUARE_WIDTH);
                let y = inner.y + (rank_idx as u16 * SQUARE_HEIGHT);

                // Check if this square is selected
                let is_selected = self
                    .app_state
                    .ui_state
                    .selected_square
                    .map(|s| s == square)
                    .unwrap_or(false);

                // Check if this square is highlighted (legal move destination)
                let is_highlighted = self
                    .app_state
                    .ui_state
                    .highlighted_squares
                    .contains(&square);

                // Check if this is part of the last move
                let is_last_move = self
                    .app_state
                    .ui_state
                    .last_move
                    .map(|(from, to)| from == square || to == square)
                    .unwrap_or(false);

                let is_light = (file_idx + rank_idx) % 2 == 0;

                let bg_color = if is_selected {
                    Color::Yellow
                } else if is_highlighted {
                    Color::Green
                } else if is_last_move {
                    Color::Blue
                } else if is_light {
                    Color::Rgb(240, 217, 181) // Light square
                } else {
                    Color::Rgb(181, 136, 99) // Dark square
                };

                // Draw the square background
                render_square(buf, x, y, bg_color, inner);

                // Get piece at this square
                let piece = self.app_state.game.position().piece_on(square);
                let piece_color = self.app_state.game.position().color_on(square);

                // Draw piece
                if let (Some(piece), Some(piece_color)) = (piece, piece_color) {
                    render_piece(buf, x, y, piece, piece_color, bg_color, inner);
                }
            }
        }
    }
}

fn render_square(buf: &mut Buffer, x: u16, y: u16, bg_color: Color, bounds: Rect) {
    let style = Style::default().bg(bg_color);

    for dy in 0..SQUARE_HEIGHT {
        for dx in 0..SQUARE_WIDTH {
            let px = x + dx;
            let py = y + dy;
            if px < bounds.right() && py < bounds.bottom() {
                buf.get_mut(px, py).set_style(style);
            }
        }
    }
}

fn render_piece(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    piece: Piece,
    color: ChessColor,
    bg_color: Color,
    bounds: Rect,
) {
    // Get piece representation (top line, middle line, bottom line)
    let (top, mid, bot) = piece_ascii_art(piece, color);

    let fg_color = match color {
        ChessColor::White => Color::White,
        ChessColor::Black => Color::Black,
    };

    let style = Style::default()
        .bg(bg_color)
        .fg(fg_color)
        .add_modifier(Modifier::BOLD);

    // Center the piece in the square
    let piece_y = y + 0; // Start at top of square

    // Render three lines of piece art
    if piece_y < bounds.bottom() {
        let px = x + 1;
        if px < bounds.right() {
            buf.set_string(px, piece_y, top, style);
        }
    }

    if piece_y + 1 < bounds.bottom() {
        let px = x + 0;
        if px < bounds.right() {
            buf.set_string(px, piece_y + 1, mid, style);
        }
    }

    if piece_y + 2 < bounds.bottom() {
        let px = x + 1;
        if px < bounds.right() {
            buf.set_string(px, piece_y + 2, bot, style);
        }
    }
}

fn piece_ascii_art(piece: Piece, color: ChessColor) -> (&'static str, &'static str, &'static str) {
    // ASCII art for pieces (3 lines high, fits in 6-char width)
    match (color, piece) {
        (ChessColor::White, Piece::King) => (
            " ╔═╗ ",
            " ║K║ ",
            " ╚═╝ ",
        ),
        (ChessColor::White, Piece::Queen) => (
            " ♕♕♕ ",
            " ║Q║ ",
            " ╚═╝ ",
        ),
        (ChessColor::White, Piece::Rook) => (
            " ┌┬┐ ",
            " │R│ ",
            " └─┘ ",
        ),
        (ChessColor::White, Piece::Bishop) => (
            "  △  ",
            " ║B║ ",
            " ╚═╝ ",
        ),
        (ChessColor::White, Piece::Knight) => (
            " ∩╗  ",
            " ║N║ ",
            " ╚═╝ ",
        ),
        (ChessColor::White, Piece::Pawn) => (
            "  ●  ",
            " ║P║ ",
            " ╚═╝ ",
        ),
        (ChessColor::Black, Piece::King) => (
            " ╔═╗ ",
            " ║K║ ",
            " ╚═╝ ",
        ),
        (ChessColor::Black, Piece::Queen) => (
            " ♛♛♛ ",
            " ║Q║ ",
            " ╚═╝ ",
        ),
        (ChessColor::Black, Piece::Rook) => (
            " ┌┬┐ ",
            " │R│ ",
            " └─┘ ",
        ),
        (ChessColor::Black, Piece::Bishop) => (
            "  △  ",
            " ║B║ ",
            " ╚═╝ ",
        ),
        (ChessColor::Black, Piece::Knight) => (
            " ∩╗  ",
            " ║N║ ",
            " ╚═╝ ",
        ),
        (ChessColor::Black, Piece::Pawn) => (
            "  ●  ",
            " ║P║ ",
            " ╚═╝ ",
        ),
    }
}
