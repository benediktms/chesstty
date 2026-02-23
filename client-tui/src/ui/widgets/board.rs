use super::board_overlay::{BoardOverlay, OverlayColor, OverlayElement};
use cozy_chess::{Board, Color as ChessColor, File, Piece, Rank, Square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
// Default board square colors (tan/brown)
const LIGHT_SQUARE: Color = Color::Rgb(240, 217, 181);
const DARK_SQUARE: Color = Color::Rgb(181, 136, 99);

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoardSizeVariant {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy)]
struct BoardSize {
    variant: BoardSizeVariant,
    square_width: u16,
    square_height: u16,
}

impl BoardSize {
    const SMALL: Self = Self {
        variant: BoardSizeVariant::Small,
        square_width: 9,
        square_height: 5,
    };

    const MEDIUM: Self = Self {
        variant: BoardSizeVariant::Medium,
        square_width: 13,
        square_height: 7,
    };

    const LARGE: Self = Self {
        variant: BoardSizeVariant::Large,
        square_width: 17,
        square_height: 9,
    };

    /// Calculate the best board size for the given area
    fn for_area(area: Rect) -> Self {
        let available_width = area.width.saturating_sub(4); // Account for borders
        let available_height = area.height.saturating_sub(4); // Account for borders and labels

        // Calculate required size for each variant (8 squares)
        let large_width = Self::LARGE.square_width * 8;
        let large_height = Self::LARGE.square_height * 8;

        let medium_width = Self::MEDIUM.square_width * 8;
        let medium_height = Self::MEDIUM.square_height * 8;

        if available_width >= large_width && available_height >= large_height {
            Self::LARGE
        } else if available_width >= medium_width && available_height >= medium_height {
            Self::MEDIUM
        } else {
            Self::SMALL
        }
    }

    /// Get the minimum required dimensions for this board size
    #[allow(dead_code)]
    pub fn min_dimensions(&self) -> (u16, u16) {
        (
            self.square_width * 8 + 8,  // 8 squares + borders + rank labels + padding
            self.square_height * 8 + 6, // 8 squares + borders + file labels + padding
        )
    }
}

pub struct BoardWidget<'a> {
    pub board: &'a Board,
    pub overlay: &'a BoardOverlay,
    pub flipped: bool,
}

impl<'a> BoardWidget<'a> {
    #[allow(dead_code)]
    pub fn new(board: &'a Board, overlay: &'a BoardOverlay) -> Self {
        Self {
            board,
            overlay,
            flipped: false,
        }
    }

    /// Get minimum board dimensions
    #[allow(dead_code)]
    pub fn min_dimensions() -> (u16, u16) {
        BoardSize::SMALL.min_dimensions()
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

        // Calculate the best board size for available space
        let board_size = BoardSize::for_area(inner);

        // Calculate actual board dimensions (including space for labels)
        let board_width = board_size.square_width * 8;
        let board_height = board_size.square_height * 8;

        // Account for rank labels on the left (need 3 chars) and file labels below (need 2 lines)
        let total_width = board_width + 3; // board + rank labels
        let total_height = board_height + 2; // board + file labels

        // Center the board within the available area
        let offset_x = (inner.width.saturating_sub(total_width)) / 2;
        let offset_y = (inner.height.saturating_sub(total_height)) / 2;

        // Add space for rank labels on the left
        let board_start_x = inner.x + offset_x + 3;
        let board_start_y = inner.y + offset_y;

        // Draw rank labels on the left
        for rank_idx in 0..8 {
            let y = board_start_y + (rank_idx as u16 * board_size.square_height) + 2;
            if y < inner.bottom() {
                let rank_num = if self.flipped {
                    rank_idx + 1
                } else {
                    8 - rank_idx
                };
                let rank_label = format!("{} ", rank_num);
                buf.set_string(
                    board_start_x.saturating_sub(2),
                    y,
                    &rank_label,
                    Style::default().fg(Color::Yellow),
                );
            }
        }

        // Draw file labels at the bottom
        for file_idx in 0..8 {
            let x = board_start_x + (file_idx as u16 * board_size.square_width) + 2;
            let y = board_start_y + (8 * board_size.square_height);
            if x < area.right() && y < area.bottom() {
                let file_char = if self.flipped {
                    (b'h' - file_idx as u8) as char
                } else {
                    (b'a' + file_idx as u8) as char
                };
                let file_label = format!("{}", file_char);
                buf.set_string(x, y, &file_label, Style::default().fg(Color::Yellow));
            }
        }

        // Pre-compute arrow paths for BFS rendering
        let arrow_paths = compute_arrow_paths(self.overlay, board_size, self.flipped);

        // Draw each square
        for rank_idx in 0..8 {
            for file_idx in 0..8 {
                let file = if self.flipped {
                    File::index(7 - file_idx)
                } else {
                    File::index(file_idx)
                };
                let rank = if self.flipped {
                    Rank::index(rank_idx)
                } else {
                    Rank::index(7 - rank_idx)
                };
                let square = Square::new(file, rank);

                let x = board_start_x + (file_idx as u16 * board_size.square_width);
                let y = board_start_y + (rank_idx as u16 * board_size.square_height);

                let is_light_square = (file_idx + rank_idx) % 2 == 0;

                // Resolve background color from overlay (or default board color)
                let bg_color = match self.overlay.square_tint(square) {
                    Some(color) => color.resolve(is_light_square),
                    None => {
                        if is_light_square {
                            LIGHT_SQUARE
                        } else {
                            DARK_SQUARE
                        }
                    }
                };

                // Draw the square background
                render_square(buf, x, y, bg_color, board_size, inner);

                // Get piece at this square
                let piece = self.board.piece_on(square);
                let piece_color = self.board.color_on(square);

                // Draw piece
                if let (Some(piece), Some(piece_color)) = (piece, piece_color) {
                    render_piece(
                        buf,
                        &PieceRenderParams {
                            x,
                            y,
                            piece,
                            color: piece_color,
                            bg_color,
                            board_size,
                            bounds: inner,
                        },
                    );
                }

                // Draw outline (border) around square if present
                if let Some(outline_color) = self.overlay.square_outline(square) {
                    draw_square_outline(
                        buf,
                        x,
                        y,
                        outline_color.resolve(is_light_square),
                        board_size,
                        inner,
                    );
                }
            }
        }

        // Draw arrow paths on top of everything
        for arrow_path in &arrow_paths {
            render_arrow_path(
                buf,
                arrow_path,
                board_start_x,
                board_start_y,
                board_size,
                inner,
            );
        }
    }
}

fn render_square(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    bg_color: Color,
    board_size: BoardSize,
    bounds: Rect,
) {
    let style = Style::default().bg(bg_color);

    for dy in 0..board_size.square_height {
        for dx in 0..board_size.square_width {
            let px = x + dx;
            let py = y + dy;
            if px < bounds.right() && py < bounds.bottom() {
                buf[(px, py)].set_style(style);
            }
        }
    }
}

struct PieceRenderParams {
    x: u16,
    y: u16,
    piece: Piece,
    color: ChessColor,
    bg_color: Color,
    board_size: BoardSize,
    bounds: Rect,
}

fn render_piece(buf: &mut Buffer, params: &PieceRenderParams) {
    // Get piece representation
    let lines = piece_pixel_art(params.piece, params.board_size.variant);

    let fg_color = match params.color {
        ChessColor::White => Color::White,
        ChessColor::Black => Color::Black,
    };

    let style = Style::default()
        .bg(params.bg_color)
        .fg(fg_color)
        .add_modifier(Modifier::BOLD);

    // Render each line of piece art, centered
    for (i, line) in lines.iter().enumerate() {
        let py = params.y + i as u16;
        if py < params.bounds.bottom() {
            // Center the text in the square
            let line_width = line.chars().count() as u16;
            let offset = (params.board_size.square_width.saturating_sub(line_width)) / 2;
            let px = params.x + offset;
            if px < params.bounds.right() {
                buf.set_string(px, py, line, style);
            }
        }
    }
}

fn piece_pixel_art(piece: Piece, size: BoardSizeVariant) -> Vec<&'static str> {
    match size {
        BoardSizeVariant::Small => piece_pixel_art_small(piece),
        BoardSizeVariant::Medium => piece_pixel_art_medium(piece),
        BoardSizeVariant::Large => piece_pixel_art_large(piece),
    }
}

#[rustfmt::skip]
fn piece_pixel_art_small(piece: Piece) -> Vec<&'static str> {
    // 4 lines high, fits in 9-char width
    match piece {
        Piece::King => vec![
            "  ✺▲✺▲✺  ",
            "   ███   ",
            "  -=K=-  ",
            "  █████  ",
        ],
        Piece::Queen => vec![
            " ✦◣◢✦◣◢✦ ",
            "   ███   ",
            "  -=Q=-  ",
            "  █████  ",
        ],
        Piece::Rook => vec![
            "  █ █ █  ",
            "   ███   ",
            "  -=R=-  ",
            "  █████  ",
        ],
        Piece::Bishop => vec![
            "    ❂    ",
            "  ▓███▓  ",
            "  -=B=-  ",
            "  █████  ",
        ],
        Piece::Knight => vec![
            "    ◉    ",
            "   ▓██▓  ",
            "  -=N=-  ",
            "  █████  ",
        ],
        Piece::Pawn => vec![
            "    ●    ",
            "   ▓▓▓   ",
            "  -=P=-  ",
            "  █████  ",
        ],
    }
}

#[rustfmt::skip]
fn piece_pixel_art_medium(piece: Piece) -> Vec<&'static str> {
    // 6 lines high, fits in 13-char width
    match piece {
        Piece::King => vec![
            "   ✺█✺█✺█✺   ",
            "   ███████   ",
            "   ▓█████▓   ",
            "  ---=K=---  ",
            "    █▓▓██    ",
            "  █████████  ",
        ],
        Piece::Queen => vec![
            "  ◣✦◣◢✦◣◢✦◢  ",
            "   ▓██████   ",
            "   ▓█████▓   ",
            "  ---=Q=---  ",
            "   ▓█▓▓██    ",
            "  ▓████████  ",
        ],
        Piece::Rook => vec![
            "  █ █ █ █ █  ",
            "  ▓████████  ",
            "  ▓▓██████▓  ",
            "   --=R=--   ",
            "   ▌██▓▓█▐   ",
            "  ▓████████  ",
        ],
        Piece::Bishop => vec![
            "      ❂      ",
            "    ▓███▓    ",
            "   ▓██████   ",
            "   --=B=--   ",
            "   ▌██▓▓█▐   ",
            "  █████████  ",
        ],
        Piece::Knight => vec![
            "    ◉        ",
            "   ▓██▓      ",
            "   ▓▓█████   ",
            "   --=N=--   ",
            "   ▌████▐    ",
            "  ▓████████  ",
        ],
        Piece::Pawn => vec![
            "      ●      ",
            "     ▓▓▓     ",
            "   ███████   ",
            "   --=P=--   ",
            "    ▄▓▓▓▄    ",
            "  █████████  "
        ],
    }
}

#[rustfmt::skip]
fn piece_pixel_art_large(piece: Piece) -> Vec<&'static str> {
    // 8 lines high, fits in 17-char width
    match piece {
        Piece::King => vec![
            "    ✺█✺█✺█✺█✺    ",
            "    █████████    ",
            "   ▓█████████▓   ",
            "    ---=K=---    ",
            "   ▌█████████▐   ",
            "    ▌██▓▓▓██▐    ",
            "     ██   ██     ",
            "   ███████████   ",
        ],
        Piece::Queen => vec![
            "   ◣✦◢◣◢✦◣◢◣✦◢   ",
            "    ▓███████▓    ",
            "   ▌█████████▐   ",
            "    ---=Q=---    ",
            "    ▌██▓▓▓██▐    ",
            "     ██   ██     ",
            "   ███████████   ",
        ],
        Piece::Rook => vec![
            "  █ █ █ █ █ █ █  ",
            "   ▓▓█████████   ",
            "   ▓▓▓████████   ",
            "    ---=R=---    ",
            "   ▌▌███████▐▐  ",
            "   ▌▌█▓▓█▓▓█▐▐   ",
            "    ▌███████▐    ",
            "   ███████████   ",
        ],
        Piece::Bishop => vec![
            "       ❂         ",
            "     ▓███▓       ",
            "    ▓█████▓      ",
            "   ---=B=---     ",
            "   ▓██████▓      ",
            "   ▌██▓▓▓██▐     ",
            "    ██   ██      ",
            "  ███████████    ",
        ],
        Piece::Knight => vec![
            "      ◉          ",
            "    ▓██▓▓        ",
            "   ▓▓█████▓▓     ",
            "   --=N=--       ",
            "   ▓▓█████       ",
            "   ▌██▓▓▓██▐     ",
            "    ██   ██      ",
            "  ███████████    ",
        ],
        Piece::Pawn => vec![
            "        ●        ",
            "       ▓▓▓       ",
            "      ▓████      ",
            "    ---=P=---    ",
            "     ▓▓▓▓▓▓▓     ",
            "     ▄▄▄▄▄▄▄     ",
            "     ▓█   ██     ",
            "    █████████    ",
        ],
    }
}

/// Draw an outline (border) around a square.
fn draw_square_outline(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    color: Color,
    board_size: BoardSize,
    bounds: Rect,
) {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    // Draw top border
    for dx in 0..board_size.square_width {
        let px = x + dx;
        if px < bounds.right() && y < bounds.bottom() {
            let symbol = if dx == 0 {
                "┌"
            } else if dx == board_size.square_width - 1 {
                "┐"
            } else {
                "─"
            };
            buf[(px, y)].set_symbol(symbol).set_style(style);
        }
    }

    // Draw bottom border
    let bottom_y = y + board_size.square_height - 1;
    for dx in 0..board_size.square_width {
        let px = x + dx;
        if px < bounds.right() && bottom_y < bounds.bottom() {
            let symbol = if dx == 0 {
                "└"
            } else if dx == board_size.square_width - 1 {
                "┘"
            } else {
                "─"
            };
            buf[(px, bottom_y)].set_symbol(symbol).set_style(style);
        }
    }

    // Draw left and right borders
    for dy in 1..board_size.square_height - 1 {
        let py = y + dy;
        if x < bounds.right() && py < bounds.bottom() {
            buf[(x, py)].set_symbol("│").set_style(style);
        }
        let right_x = x + board_size.square_width - 1;
        if right_x < bounds.right() && py < bounds.bottom() {
            buf[(right_x, py)].set_symbol("│").set_style(style);
        }
    }
}

// =========================================================================
// BFS Arrow Rendering
// =========================================================================

/// A computed arrow path through the pixel grid, ready for rendering.
struct ArrowPath {
    /// Sequence of (px, py) pixel positions forming the arrow body.
    cells: Vec<(u16, u16)>,
    /// The arrow color.
    color: OverlayColor,
    /// The pixel position of the arrow head.
    head: Option<(u16, u16)>,
    /// Direction of the arrow at its head (for choosing the head symbol).
    head_direction: (i16, i16),
}

/// Compute arrow paths for all Arrow overlay elements using BFS.
///
/// Each arrow is traced from the center of the `from` square to the center
/// of the `to` square through the terminal's cell grid.
fn compute_arrow_paths(
    overlay: &BoardOverlay,
    board_size: BoardSize,
    flipped: bool,
) -> Vec<ArrowPath> {
    let mut paths = Vec::new();

    for element in overlay.elements() {
        if let OverlayElement::Arrow {
            from, to, color, ..
        } = element
        {
            let (from_col, from_row) = square_to_grid_idx(*from, flipped);
            let (to_col, to_row) = square_to_grid_idx(*to, flipped);

            // Center pixel of each square (relative to board_start)
            let from_cx = from_col as u16 * board_size.square_width + board_size.square_width / 2;
            let from_cy = from_row as u16 * board_size.square_height + board_size.square_height / 2;
            let to_cx = to_col as u16 * board_size.square_width + board_size.square_width / 2;
            let to_cy = to_row as u16 * board_size.square_height + board_size.square_height / 2;

            let cells = bfs_line(from_cx, from_cy, to_cx, to_cy);
            let head_direction = if cells.len() >= 2 {
                let (px, py) = cells[cells.len() - 1];
                let (ppx, ppy) = cells[cells.len() - 2];
                (px as i16 - ppx as i16, py as i16 - ppy as i16)
            } else {
                (0, 0)
            };
            let head = cells.last().copied();

            paths.push(ArrowPath {
                cells,
                color: *color,
                head,
                head_direction,
            });
        }
    }

    paths
}

/// Convert a chess square to grid indices (col, row) where (0,0) is top-left.
fn square_to_grid_idx(square: Square, flipped: bool) -> (usize, usize) {
    let file_idx = square.file() as usize;
    let rank_idx = square.rank() as usize;

    if flipped {
        (7 - file_idx, rank_idx)
    } else {
        (file_idx, 7 - rank_idx)
    }
}

/// BFS/Bresenham-like line from (x0, y0) to (x1, y1) through the cell grid.
/// Returns a list of (x, y) pixel coordinates forming the path.
///
/// We use a simple Bresenham line algorithm which traces the shortest
/// straight path — this is more efficient than BFS for straight/diagonal arrows
/// and produces clean visual lines.
fn bfs_line(x0: u16, y0: u16, x1: u16, y1: u16) -> Vec<(u16, u16)> {
    let mut cells = Vec::new();

    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut cx = x0 as i32;
    let mut cy = y0 as i32;

    loop {
        cells.push((cx as u16, cy as u16));

        if cx == x1 as i32 && cy == y1 as i32 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }

    cells
}

/// Render a computed arrow path onto the buffer.
fn render_arrow_path(
    buf: &mut Buffer,
    path: &ArrowPath,
    board_start_x: u16,
    board_start_y: u16,
    board_size: BoardSize,
    bounds: Rect,
) {
    if path.cells.is_empty() {
        return;
    }

    // Determine which cells to render: skip the first and last few cells
    // (inside the from/to squares) to avoid overwriting piece art.
    // We render the arrow body through the gap between squares.
    let skip_start = (board_size.square_width.min(board_size.square_height) / 3) as usize;
    let skip_end = skip_start;

    let total = path.cells.len();
    if total <= skip_start + skip_end {
        return; // Arrow too short to render a visible path
    }

    // Use the overlay color resolved for a neutral context (dark)
    let arrow_fg = path.color.resolve(false);
    let style = Style::default().fg(arrow_fg).add_modifier(Modifier::BOLD);

    // Draw arrow body
    for &(px, py) in &path.cells[skip_start..total.saturating_sub(skip_end)] {
        let screen_x = board_start_x + px;
        let screen_y = board_start_y + py;
        if screen_x < bounds.right()
            && screen_y < bounds.bottom()
            && screen_x >= bounds.x
            && screen_y >= bounds.y
        {
            let (dx, dy) = path.head_direction;
            let symbol = arrow_body_symbol(dx, dy);
            buf[(screen_x, screen_y)]
                .set_symbol(symbol)
                .set_style(style);
        }
    }

    // Draw arrow head
    if let Some((hx, hy)) = path.head {
        let screen_x = board_start_x + hx;
        let screen_y = board_start_y + hy;
        if screen_x < bounds.right()
            && screen_y < bounds.bottom()
            && screen_x >= bounds.x
            && screen_y >= bounds.y
        {
            let (dx, dy) = path.head_direction;
            let symbol = arrow_head_symbol(dx, dy);
            let head_style = Style::default().fg(arrow_fg).add_modifier(Modifier::BOLD);
            buf[(screen_x, screen_y)]
                .set_symbol(symbol)
                .set_style(head_style);
        }
    }
}

/// Choose an arrow body character based on the overall direction.
fn arrow_body_symbol(dx: i16, dy: i16) -> &'static str {
    let dx = dx.signum();
    let dy = dy.signum();
    match (dx, dy) {
        (0, _) => "│",            // vertical
        (_, 0) => "─",            // horizontal
        (1, -1) | (-1, 1) => "╱", // diagonal /
        (1, 1) | (-1, -1) => "╲", // diagonal \
        _ => "·",
    }
}

/// Choose an arrow head character based on direction.
fn arrow_head_symbol(dx: i16, dy: i16) -> &'static str {
    let dx = dx.signum();
    let dy = dy.signum();
    match (dx, dy) {
        (0, -1) => "▲",  // up (rank increases = visually up when not flipped)
        (0, 1) => "▼",   // down
        (1, 0) => "▶",   // right
        (-1, 0) => "◀",  // left
        (1, -1) => "◥",  // up-right
        (-1, -1) => "◤", // up-left
        (1, 1) => "◢",   // down-right
        (-1, 1) => "◣",  // down-left
        _ => "●",
    }
}
