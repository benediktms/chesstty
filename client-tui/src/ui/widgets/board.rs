use crate::state::ClientState;
use cozy_chess::{Color as ChessColor, File, Piece, Rank, Square};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

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

        let _small_width = Self::SMALL.square_width * 8;
        let _small_height = Self::SMALL.square_height * 8;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SquareHighlight {
    Selected,
    LegalMove,
    BestMove,
    LastMove,
    Typeahead,
    None,
}

impl SquareHighlight {
    fn bg_color(self, is_light_square: bool) -> Color {
        let (light, dark) = match self {
            Self::Selected => (Color::LightYellow, Color::Yellow),
            Self::LegalMove => (Color::LightBlue, Color::Blue),
            Self::BestMove => (Color::LightGreen, Color::Green),
            Self::LastMove => (Color::LightYellow, Color::Yellow),
            Self::Typeahead => (Color::LightCyan, Color::Cyan),
            Self::None => (Color::Rgb(240, 217, 181), Color::Rgb(181, 136, 99)),
        };
        if is_light_square {
            light
        } else {
            dark
        }
    }
}

pub struct BoardWidget<'a> {
    pub client_state: &'a ClientState,
    pub typeahead_squares: &'a [Square],
    pub flipped: bool,
}

impl<'a> BoardWidget<'a> {
    #[allow(dead_code)]
    pub fn new(client_state: &'a ClientState, typeahead_squares: &'a [Square]) -> Self {
        Self {
            client_state,
            typeahead_squares,
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

        // Draw each square
        for rank_idx in 0..8 {
            for file_idx in 0..8 {
                let file = if self.flipped {
                    File::index(7 - file_idx)
                } else {
                    File::index(file_idx)
                };
                let rank = if self.flipped {
                    Rank::index(rank_idx) // Bottom rank (1) is at top when flipped
                } else {
                    Rank::index(7 - rank_idx) // Top rank is 8
                };
                let square = Square::new(file, rank);

                let x = board_start_x + (file_idx as u16 * board_size.square_width);
                let y = board_start_y + (rank_idx as u16 * board_size.square_height);

                // Check if this square is selected
                let is_selected = self
                    .client_state
                    .ui
                    .selected_square
                    .map(|s| s == square)
                    .unwrap_or(false);

                // Check if this square is highlighted (legal move destination)
                let is_highlighted = self.client_state.ui.highlighted_squares.contains(&square);

                // Check if this is part of the last move
                let is_last_move = self
                    .client_state
                    .ui
                    .last_move
                    .map(|(from, to)| from == square || to == square)
                    .unwrap_or(false);

                // Check if this square matches typeahead input
                let is_typeahead = self.typeahead_squares.contains(&square);

                // Check if this is a best move square (review mode)
                let is_best_move = self
                    .client_state
                    .ui
                    .best_move_squares
                    .map(|(from, to)| from == square || to == square)
                    .unwrap_or(false);

                let is_light_square = (file_idx + rank_idx) % 2 == 0;

                let highlight = if is_selected {
                    SquareHighlight::Selected
                } else if is_highlighted {
                    SquareHighlight::LegalMove
                } else if is_best_move {
                    SquareHighlight::BestMove
                } else if is_last_move {
                    SquareHighlight::LastMove
                } else if is_typeahead {
                    SquareHighlight::Typeahead
                } else {
                    SquareHighlight::None
                };

                let bg_color = highlight.bg_color(is_light_square);

                // Draw the square background (with highlight color if applicable)
                render_square(buf, x, y, bg_color, board_size, inner);

                // Get piece at this square
                let piece = self.client_state.board().piece_on(square);
                let piece_color = self.client_state.board().color_on(square);

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

                // Check for tab-selected destination and draw magenta outline
                let tab_selected_destination = get_tab_selected_square(self.client_state);
                let is_tab_selected = tab_selected_destination
                    .map(|s| s == square)
                    .unwrap_or(false);

                if is_tab_selected {
                    draw_square_outline(buf, x, y, Color::Magenta, board_size, inner);
                }
            }
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
        // ChessColor::Black => Color::Rgb(50, 50, 50), // Dark gray for black pieces
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
            "  ✺█✺█✺  ",
            "   ███   ",
            "  -=K=-  ",
            "  █████  ",
        ],
        Piece::Queen => vec![
            "  ✦█✦█✦  ",
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
            "  ✦█✦█✦█✦█✦  ",
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
            "   ✦█✦█✦█✦█✦█✦   ",
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

/// Get the currently selected destination square in tab input mode.
fn get_tab_selected_square(_state: &ClientState) -> Option<Square> {
    // No arrow-based selection — typeahead only
    None
}

/// Draw a magenta outline around a square.
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
        if x < bounds.bottom() {
            buf[(x, py)].set_symbol("│").set_style(style);
        }
        let right_x = x + board_size.square_width - 1;
        if right_x < bounds.right() {
            buf[(right_x, py)].set_symbol("│").set_style(style);
        }
    }
}
