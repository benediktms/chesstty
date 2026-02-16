//! Canonical piece and color types for the project.
//! cozy-chess types are internal implementation details.

/// Project-owned piece type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// Project-owned color type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceColor {
    White,
    Black,
}

impl PieceKind {
    pub fn to_char_upper(self) -> char {
        match self {
            Self::Pawn => 'P',
            Self::Knight => 'N',
            Self::Bishop => 'B',
            Self::Rook => 'R',
            Self::Queen => 'Q',
            Self::King => 'K',
        }
    }

    pub fn to_char_lower(self) -> char {
        match self {
            Self::Pawn => 'p',
            Self::Knight => 'n',
            Self::Bishop => 'b',
            Self::Rook => 'r',
            Self::Queen => 'q',
            Self::King => 'k',
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'p' => Some(Self::Pawn),
            'n' => Some(Self::Knight),
            'b' => Some(Self::Bishop),
            'r' => Some(Self::Rook),
            'q' => Some(Self::Queen),
            'k' => Some(Self::King),
            _ => None,
        }
    }
}

impl PieceColor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Black => "black",
        }
    }
}

impl From<cozy_chess::Piece> for PieceKind {
    fn from(p: cozy_chess::Piece) -> Self {
        match p {
            cozy_chess::Piece::Pawn => Self::Pawn,
            cozy_chess::Piece::Knight => Self::Knight,
            cozy_chess::Piece::Bishop => Self::Bishop,
            cozy_chess::Piece::Rook => Self::Rook,
            cozy_chess::Piece::Queen => Self::Queen,
            cozy_chess::Piece::King => Self::King,
        }
    }
}

impl From<PieceKind> for cozy_chess::Piece {
    fn from(p: PieceKind) -> Self {
        match p {
            PieceKind::Pawn => Self::Pawn,
            PieceKind::Knight => Self::Knight,
            PieceKind::Bishop => Self::Bishop,
            PieceKind::Rook => Self::Rook,
            PieceKind::Queen => Self::Queen,
            PieceKind::King => Self::King,
        }
    }
}

impl From<cozy_chess::Color> for PieceColor {
    fn from(c: cozy_chess::Color) -> Self {
        match c {
            cozy_chess::Color::White => Self::White,
            cozy_chess::Color::Black => Self::Black,
        }
    }
}

impl From<PieceColor> for cozy_chess::Color {
    fn from(c: PieceColor) -> Self {
        match c {
            PieceColor::White => Self::White,
            PieceColor::Black => Self::Black,
        }
    }
}

impl std::fmt::Display for PieceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_char_upper())
    }
}

impl std::fmt::Display for PieceColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
