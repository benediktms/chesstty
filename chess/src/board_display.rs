//! Lightweight board representation for rendering from FEN.

use crate::types::{PieceColor, PieceKind};

/// An 8x8 board for display purposes only.
#[derive(Debug, Clone, Default)]
pub struct DisplayBoard {
    squares: [[Option<(PieceKind, PieceColor)>; 8]; 8],
}

impl DisplayBoard {
    /// Parse the board placement from a FEN string.
    pub fn from_fen(fen: &str) -> Result<Self, DisplayBoardError> {
        let placement = fen
            .split_whitespace()
            .next()
            .ok_or(DisplayBoardError::InvalidFen)?;

        let mut squares = [[None; 8]; 8];
        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(DisplayBoardError::InvalidFen);
        }

        for (rank_idx, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - rank_idx;
            let mut file = 0usize;
            for c in rank_str.chars() {
                if file > 7 {
                    return Err(DisplayBoardError::InvalidFen);
                }
                if let Some(skip) = c.to_digit(10) {
                    file += skip as usize;
                } else {
                    let color = if c.is_uppercase() {
                        PieceColor::White
                    } else {
                        PieceColor::Black
                    };
                    let kind = PieceKind::from_char(c).ok_or(DisplayBoardError::InvalidPiece(c))?;
                    squares[rank][file] = Some((kind, color));
                    file += 1;
                }
            }
        }

        Ok(DisplayBoard { squares })
    }

    pub fn piece_at(&self, file: u8, rank: u8) -> Option<(PieceKind, PieceColor)> {
        if file > 7 || rank > 7 {
            return None;
        }
        self.squares[rank as usize][file as usize]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DisplayBoardError {
    #[error("Invalid FEN string")]
    InvalidFen,
    #[error("Invalid piece character: {0}")]
    InvalidPiece(char),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position() {
        let board =
            DisplayBoard::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
                .unwrap();
        assert_eq!(
            board.piece_at(0, 0),
            Some((PieceKind::Rook, PieceColor::White))
        );
        assert_eq!(
            board.piece_at(4, 0),
            Some((PieceKind::King, PieceColor::White))
        );
        assert_eq!(
            board.piece_at(3, 7),
            Some((PieceKind::Queen, PieceColor::Black))
        );
        assert_eq!(board.piece_at(4, 4), None);
    }

    #[test]
    fn test_empty_board() {
        let board = DisplayBoard::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").unwrap();
        for rank in 0..8 {
            for file in 0..8 {
                assert_eq!(board.piece_at(file, rank), None);
            }
        }
    }
}
