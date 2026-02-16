//! Parsing functions from protobuf types to domain types

use ::chess::{format_square, parse_file, parse_piece, parse_rank, parse_square};
use chess_proto::*;
use cozy_chess::{File as CozyFile, Move, Piece, Rank, Square};
use tonic::Status;

pub fn parse_move_repr(mv: &MoveRepr) -> Result<Move, Status> {
    let from = parse_square_grpc(&mv.from)?;
    let to = parse_square_grpc(&mv.to)?;
    let promotion = if let Some(ref p) = mv.promotion {
        if p.len() == 1 {
            let c = p.chars().next().unwrap();
            Some(parse_piece_grpc(c)?)
        } else {
            return Err(Status::invalid_argument(format!("Invalid piece: {}", p)));
        }
    } else {
        None
    };

    Ok(Move {
        from,
        to,
        promotion,
    })
}

pub fn parse_square_grpc(s: &str) -> Result<Square, Status> {
    parse_square(s).ok_or_else(|| Status::invalid_argument(format!("Invalid square: {}", s)))
}

pub fn parse_file_grpc(c: char) -> Result<CozyFile, Status> {
    parse_file(c).ok_or_else(|| Status::invalid_argument(format!("Invalid file: {}", c)))
}

pub fn parse_rank_grpc(c: char) -> Result<Rank, Status> {
    parse_rank(c).ok_or_else(|| Status::invalid_argument(format!("Invalid rank: {}", c)))
}

pub fn parse_piece_grpc(c: char) -> Result<Piece, Status> {
    parse_piece(c).ok_or_else(|| Status::invalid_argument(format!("Invalid piece: {}", c)))
}

pub fn format_move_san(mv: &Move) -> String {
    // Simplified SAN format (just from-to notation)
    // A full implementation would need game context
    format!("{}{}", format_square(mv.from), format_square(mv.to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_square_grpc_valid() {
        let sq = parse_square_grpc("e4").unwrap();
        assert_eq!(sq.file(), CozyFile::E);
        assert_eq!(sq.rank(), Rank::Fourth);
    }

    #[test]
    fn test_parse_square_grpc_invalid() {
        let result = parse_square_grpc("z9");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_move_repr_simple() {
        let mv_repr = MoveRepr {
            from: "e2".to_string(),
            to: "e4".to_string(),
            promotion: None,
        };
        let mv = parse_move_repr(&mv_repr).unwrap();
        assert_eq!(mv.from.file(), CozyFile::E);
        assert_eq!(mv.from.rank(), Rank::Second);
        assert_eq!(mv.to.file(), CozyFile::E);
        assert_eq!(mv.to.rank(), Rank::Fourth);
        assert!(mv.promotion.is_none());
    }

    #[test]
    fn test_parse_move_repr_with_promotion() {
        let mv_repr = MoveRepr {
            from: "e7".to_string(),
            to: "e8".to_string(),
            promotion: Some("q".to_string()),
        };
        let mv = parse_move_repr(&mv_repr).unwrap();
        assert_eq!(mv.promotion, Some(Piece::Queen));
    }

    #[test]
    fn test_format_move_san() {
        let mv = Move {
            from: Square::new(CozyFile::E, Rank::Second),
            to: Square::new(CozyFile::E, Rank::Fourth),
            promotion: None,
        };
        assert_eq!(format_move_san(&mv), "e2e4");
    }
}
