// Conversion utilities between ClientState (string-based) and UI expectations (cozy-chess types)

use cozy_chess::{Color, File, Piece, Rank, Square};

/// Parse a square string like "e2" to cozy_chess::Square
pub fn parse_square(s: &str) -> Option<Square> {
    if s.len() != 2 {
        return None;
    }

    let chars: Vec<char> = s.chars().collect();
    let file = parse_file(chars[0])?;
    let rank = parse_rank(chars[1])?;

    Some(Square::new(file, rank))
}

/// Parse a file character like 'e' to cozy_chess::File
pub fn parse_file(c: char) -> Option<File> {
    match c.to_ascii_lowercase() {
        'a' => Some(File::A),
        'b' => Some(File::B),
        'c' => Some(File::C),
        'd' => Some(File::D),
        'e' => Some(File::E),
        'f' => Some(File::F),
        'g' => Some(File::G),
        'h' => Some(File::H),
        _ => None,
    }
}

/// Parse a rank character like '2' to cozy_chess::Rank
pub fn parse_rank(c: char) -> Option<Rank> {
    match c {
        '1' => Some(Rank::First),
        '2' => Some(Rank::Second),
        '3' => Some(Rank::Third),
        '4' => Some(Rank::Fourth),
        '5' => Some(Rank::Fifth),
        '6' => Some(Rank::Sixth),
        '7' => Some(Rank::Seventh),
        '8' => Some(Rank::Eighth),
        _ => None,
    }
}

/// Format a square to string like "e2"
pub fn format_square(sq: Square) -> String {
    let file = format_file(sq.file());
    let rank = format_rank(sq.rank());
    format!("{}{}", file, rank)
}

/// Format a file to character
pub fn format_file(f: File) -> char {
    match f {
        File::A => 'a',
        File::B => 'b',
        File::C => 'c',
        File::D => 'd',
        File::E => 'e',
        File::F => 'f',
        File::G => 'g',
        File::H => 'h',
    }
}

/// Format a rank to character
pub fn format_rank(r: Rank) -> char {
    match r {
        Rank::First => '1',
        Rank::Second => '2',
        Rank::Third => '3',
        Rank::Fourth => '4',
        Rank::Fifth => '5',
        Rank::Sixth => '6',
        Rank::Seventh => '7',
        Rank::Eighth => '8',
    }
}

/// Parse a color string to cozy_chess::Color
pub fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "white" => Some(Color::White),
        "black" => Some(Color::Black),
        _ => None,
    }
}

/// Format a piece to its character representation
pub fn format_piece(piece: Piece) -> char {
    match piece {
        Piece::Pawn => 'p',
        Piece::Knight => 'n',
        Piece::Bishop => 'b',
        Piece::Rook => 'r',
        Piece::Queen => 'q',
        Piece::King => 'k',
    }
}

/// Parse a piece character
pub fn parse_piece(c: char) -> Option<Piece> {
    match c.to_ascii_lowercase() {
        'p' => Some(Piece::Pawn),
        'n' => Some(Piece::Knight),
        'b' => Some(Piece::Bishop),
        'r' => Some(Piece::Rook),
        'q' => Some(Piece::Queen),
        'k' => Some(Piece::King),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_square() {
        let sq = parse_square("e2").unwrap();
        assert_eq!(sq.file(), File::E);
        assert_eq!(sq.rank(), Rank::Second);
    }

    #[test]
    fn test_format_square() {
        let sq = Square::new(File::E, Rank::Fourth);
        assert_eq!(format_square(sq), "e4");
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("white"), Some(Color::White));
        assert_eq!(parse_color("black"), Some(Color::Black));
        assert_eq!(parse_color("invalid"), None);
    }
}
