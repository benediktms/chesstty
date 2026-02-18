use cozy_chess::{BitBoard, Board, Color, File, Piece, Rank, Square};
use serde::{Deserialize, Serialize};

use super::helpers::{attacked_squares, attackers_of, king_zone_files};

/// King safety metrics for one side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KingSafetyMetrics {
    pub color: char,
    /// Number of shield pawns present (0-3).
    pub pawn_shield_count: u8,
    /// Maximum possible shield pawns (always 3).
    pub pawn_shield_max: u8,
    /// Open files near king (0-3 files without own pawns).
    pub open_files_near_king: u8,
    /// Number of enemy pieces attacking the king zone.
    pub attacker_count: u8,
    /// Weighted attack score: Q=4, R=3, B=2, N=2, P=1.
    pub attack_weight: u16,
    /// Number of king zone squares attacked by the enemy.
    pub attacked_king_zone_squares: u8,
    /// Total king zone size (king moves + king square).
    pub king_zone_size: u8,
    /// Composite exposure score from 0.0 (safe) to 1.0 (exposed).
    pub exposure_score: f32,
}

/// King safety for both sides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionKingSafety {
    pub white: KingSafetyMetrics,
    pub black: KingSafetyMetrics,
}

/// Compute king safety metrics for both sides.
pub fn compute_king_safety(board: &Board) -> PositionKingSafety {
    PositionKingSafety {
        white: compute_side_king_safety(board, Color::White),
        black: compute_side_king_safety(board, Color::Black),
    }
}

fn compute_side_king_safety(board: &Board, color: Color) -> KingSafetyMetrics {
    let enemy = !color;
    let king_bb = board.pieces(Piece::King) & board.colors(color);
    let king_sq = match king_bb.into_iter().next() {
        Some(sq) => sq,
        None => {
            return KingSafetyMetrics {
                color: if color == Color::White { 'w' } else { 'b' },
                pawn_shield_count: 0,
                pawn_shield_max: 3,
                open_files_near_king: 3,
                attacker_count: 0,
                attack_weight: 0,
                attacked_king_zone_squares: 0,
                king_zone_size: 0,
                exposure_score: 1.0,
            };
        }
    };

    // King zone = king square + king moves
    let king_moves = cozy_chess::get_king_moves(king_sq);
    let king_zone = king_moves | BitBoard::from(king_sq);
    let king_zone_size = king_zone.len() as u8;

    // Pawn shield: own pawns on the 3 files around king at expected shield rank(s)
    let own_pawns = board.pieces(Piece::Pawn) & board.colors(color);
    let shield_rank = if color == Color::White {
        Rank::Second
    } else {
        Rank::Seventh
    };
    let advanced_shield_rank = if color == Color::White {
        Rank::Third
    } else {
        Rank::Sixth
    };

    let mut pawn_shield_count: u8 = 0;
    let king_files: Vec<File> = king_zone_files(king_sq).collect();

    for file in &king_files {
        let file_bb = file_bitboard(*file);
        let shield_pawns = own_pawns & file_bb;
        let rank_bb = rank_bitboard(shield_rank) | rank_bitboard(advanced_shield_rank);
        if !(shield_pawns & rank_bb).is_empty() {
            pawn_shield_count += 1;
        }
    }

    // Open files near king: king-adjacent files without own pawns
    let mut open_files_near_king: u8 = 0;
    for file in &king_files {
        let file_bb = file_bitboard(*file);
        if (own_pawns & file_bb).is_empty() {
            open_files_near_king += 1;
        }
    }

    // Enemy attackers of king zone
    let enemy_attacks = attacked_squares(board, enemy);
    let attacked_king_zone_squares = (enemy_attacks & king_zone).len() as u8;

    // Count enemy pieces attacking king zone and compute weighted attack
    let mut attack_weight: u16 = 0;

    // Collect unique attackers of the king zone
    let mut unique_attackers = BitBoard::EMPTY;
    for sq in king_zone {
        unique_attackers |= attackers_of(board, sq, enemy);
    }
    let attacker_count = unique_attackers.len() as u8;

    // Compute weighted attack from unique attackers
    for attacker_sq in unique_attackers {
        if let Some(piece) = board.piece_on(attacker_sq) {
            attack_weight += match piece {
                Piece::Queen => 4,
                Piece::Rook => 3,
                Piece::Bishop => 2,
                Piece::Knight => 2,
                Piece::Pawn => 1,
                Piece::King => 1,
            };
        }
    }

    // Composite exposure score: 0.0 (safe) to 1.0 (exposed)
    let shield_deficit = (3.0 - pawn_shield_count as f32) / 3.0;
    let open_file_factor = open_files_near_king as f32 / 3.0;
    let attack_factor = (attack_weight as f32 / 20.0).min(1.0);
    let zone_control = if king_zone_size > 0 {
        attacked_king_zone_squares as f32 / king_zone_size as f32
    } else {
        0.0
    };

    let exposure_score =
        (0.25 * shield_deficit + 0.20 * open_file_factor + 0.30 * attack_factor + 0.25 * zone_control)
            .clamp(0.0, 1.0);

    KingSafetyMetrics {
        color: if color == Color::White { 'w' } else { 'b' },
        pawn_shield_count,
        pawn_shield_max: 3,
        open_files_near_king,
        attacker_count,
        attack_weight,
        attacked_king_zone_squares,
        king_zone_size,
        exposure_score,
    }
}

fn file_bitboard(file: File) -> BitBoard {
    let mut bb = BitBoard::EMPTY;
    for rank in Rank::ALL {
        bb |= BitBoard::from(Square::new(file, rank));
    }
    bb
}

fn rank_bitboard(rank: Rank) -> BitBoard {
    let mut bb = BitBoard::EMPTY;
    for file in File::ALL {
        bb |= BitBoard::from(Square::new(file, rank));
    }
    bb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_king_safety() {
        let board = Board::default();
        let safety = compute_king_safety(&board);

        // Both sides have full pawn shields in starting position
        // White king on e1: d2, e2, f2 pawns should count
        assert!(safety.white.pawn_shield_count >= 2);
        assert!(safety.black.pawn_shield_count >= 2);

        // Exposure should be low in starting position
        assert!(safety.white.exposure_score < 0.5);
        assert!(safety.black.exposure_score < 0.5);
    }

    #[test]
    fn test_castled_king_with_shield() {
        // White castled kingside with intact pawn shield
        let board: Board =
            "rnbqkb1r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 0 1"
                .parse()
                .unwrap();
        let safety = compute_king_safety(&board);
        // King hasn't castled yet but pawn shield should still be measured
        assert!(safety.white.pawn_shield_count >= 2);
    }

    #[test]
    fn test_exposed_king() {
        // White king on e1 with no pawns nearby
        let board: Board =
            "rnbqkbnr/pppppppp/8/8/8/8/8/4K3 w kq - 0 1".parse().unwrap();
        let safety = compute_king_safety(&board);
        assert_eq!(safety.white.pawn_shield_count, 0);
        assert!(
            safety.white.exposure_score > 0.3,
            "Exposed king should have high exposure score, got {}",
            safety.white.exposure_score
        );
    }
}
