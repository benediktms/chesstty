use crate::app::AppState;
use cozy_chess::{Color as ChessColor, File, Piece, Rank, Square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

const SQUARE_WIDTH: u16 = 9;
const SQUARE_HEIGHT: u16 = 5;

pub struct BoardWidget<'a> {
    pub app_state: &'a AppState,
    pub typeahead_squares: &'a [Square],
}

impl<'a> BoardWidget<'a> {
    pub fn new(app_state: &'a AppState, typeahead_squares: &'a [Square]) -> Self {
        Self { app_state, typeahead_squares }
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
            let y = inner.y + (rank_idx as u16 * SQUARE_HEIGHT) + 2;
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

                // Check if this square matches typeahead input
                let is_typeahead = self.typeahead_squares.contains(&square);

                let is_light = (file_idx + rank_idx) % 2 == 0;

                let bg_color = if is_light {
                    Color::Rgb(240, 217, 181) // Light square
                } else {
                    Color::Rgb(181, 136, 99) // Dark square
                };

                // Draw the square background
                render_square(buf, x, y, bg_color, inner);

                // Draw borders/outlines for highlights
                if is_selected {
                    draw_square_outline(buf, x, y, Color::Yellow, inner);
                } else if is_highlighted {
                    draw_square_outline(buf, x, y, Color::Green, inner);
                } else if is_last_move {
                    draw_square_outline(buf, x, y, Color::Blue, inner);
                } else if is_typeahead {
                    draw_square_outline(buf, x, y, Color::Cyan, inner);
                }

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

fn draw_square_outline(buf: &mut Buffer, x: u16, y: u16, color: Color, bounds: Rect) {
    let style = Style::default()
        .fg(color)
        .add_modifier(Modifier::BOLD);

    // Top border
    for dx in 0..SQUARE_WIDTH {
        let px = x + dx;
        if px < bounds.right() && y < bounds.bottom() {
            let symbol = if dx == 0 {
                "┏"
            } else if dx == SQUARE_WIDTH - 1 {
                "┓"
            } else {
                "━"
            };
            buf.get_mut(px, y).set_symbol(symbol).set_style(style);
        }
    }

    // Bottom border
    let bottom_y = y + SQUARE_HEIGHT - 1;
    for dx in 0..SQUARE_WIDTH {
        let px = x + dx;
        if px < bounds.right() && bottom_y < bounds.bottom() {
            let symbol = if dx == 0 {
                "┗"
            } else if dx == SQUARE_WIDTH - 1 {
                "┛"
            } else {
                "━"
            };
            buf.get_mut(px, bottom_y).set_symbol(symbol).set_style(style);
        }
    }

    // Left and right borders
    for dy in 1..SQUARE_HEIGHT - 1 {
        let py = y + dy;
        if py < bounds.bottom() {
            // Left border
            if x < bounds.right() {
                buf.get_mut(x, py).set_symbol("┃").set_style(style);
            }
            // Right border
            let right_x = x + SQUARE_WIDTH - 1;
            if right_x < bounds.right() {
                buf.get_mut(right_x, py).set_symbol("┃").set_style(style);
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
    // Get piece representation (5 lines for better pixel art)
    let lines = piece_pixel_art(piece, color);

    let fg_color = match color {
        ChessColor::White => Color::White,
        ChessColor::Black => Color::Rgb(50, 50, 50), // Dark gray for black pieces
    };

    let style = Style::default()
        .bg(bg_color)
        .fg(fg_color)
        .add_modifier(Modifier::BOLD);

    // Render each line of piece art, centered
    for (i, line) in lines.iter().enumerate() {
        let py = y + i as u16;
        if py < bounds.bottom() {
            // Center the text in the square
            let line_width = line.chars().count() as u16;
            let offset = (SQUARE_WIDTH.saturating_sub(line_width)) / 2;
            let px = x + offset;
            if px < bounds.right() {
                buf.set_string(px, py, line, style);
            }
        }
    }
}

fn piece_pixel_art(piece: Piece, _color: ChessColor) -> [&'static str; 5] {
    // Pixel art for pieces (5 lines high, fits in 9-char width)
    // Using Unicode block characters for better visual representation
    match piece {
        Piece::King => [
            "  ╔╦╗  ",
            " ▐███▌ ",
            " ▐███▌ ",
            " █████ ",
            "▀▀▀▀▀▀▀",
        ],
        Piece::Queen => [
            " █▀█▀█ ",
            " ▐███▌ ",
            " ▐███▌ ",
            " █████ ",
            "▀▀▀▀▀▀▀",
        ],
        Piece::Rook => [
            " █▀▀█  ",
            " ████  ",
            " ▐██▌  ",
            " ████  ",
            "▀▀▀▀▀▀ ",
        ],
        Piece::Bishop => [
            "   ▲   ",
            "  ╱█╲  ",
            "  ▐█▌  ",
            " ▐███▌ ",
            "▀▀▀▀▀▀▀",
        ],
        Piece::Knight => [
            "  ▄█▀  ",
            " ▐██▌  ",
            " ▐██   ",
            " ████  ",
            "▀▀▀▀▀▀ ",
        ],
        Piece::Pawn => [
            "   ●   ",
            "  ▄█▄  ",
            "  ███  ",
            " ▀███▀ ",
            "       ",
        ],
    }
}
