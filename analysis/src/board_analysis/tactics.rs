use cozy_chess::{BitBoard, Board, Color, File, Piece, Rank, Square};
use serde::{Deserialize, Serialize};

use super::helpers::{attacked_squares, attackers_of, piece_attacks, piece_value};

/// Information about a piece on a specific square.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquareInfo {
    pub square: String,
    pub piece: char,
    pub color: char,
}

impl SquareInfo {
    fn new(sq: Square, piece: Piece, color: Color) -> Self {
        Self {
            square: format!("{}", sq),
            piece: piece_char(piece),
            color: if color == Color::White { 'w' } else { 'b' },
        }
    }
}

fn piece_char(p: Piece) -> char {
    match p {
        Piece::Pawn => 'P',
        Piece::Knight => 'N',
        Piece::Bishop => 'B',
        Piece::Rook => 'R',
        Piece::Queen => 'Q',
        Piece::King => 'K',
    }
}

/// A detected tactical pattern on the board.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TacticalPattern {
    Fork {
        attacker: SquareInfo,
        targets: Vec<SquareInfo>,
    },
    Pin {
        pinner: SquareInfo,
        pinned_piece: SquareInfo,
        pinned_to: SquareInfo,
    },
    Skewer {
        attacker: SquareInfo,
        front_piece: SquareInfo,
        back_piece: SquareInfo,
    },
    DiscoveredAttack {
        moving_piece: SquareInfo,
        revealed_attacker: SquareInfo,
        target: SquareInfo,
    },
    HangingPiece {
        piece: SquareInfo,
        attacker_count: u8,
        defender_count: u8,
    },
    BackRankWeakness {
        king_square: SquareInfo,
        blocking_rank: u8,
    },
}

/// Summary of all tactical patterns found in a position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalAnalysis {
    pub patterns: Vec<TacticalPattern>,
    pub fork_count: u8,
    pub pin_count: u8,
    pub skewer_count: u8,
    pub discovered_attack_count: u8,
    pub hanging_piece_count: u8,
    pub has_back_rank_weakness: bool,
}

/// Analyze a board position for tactical patterns.
/// `perspective` is the side whose tactics we are detecting (the side that benefits).
pub fn analyze_tactics(board: &Board, perspective: Color) -> TacticalAnalysis {
    let mut patterns = Vec::new();

    detect_forks(board, perspective, &mut patterns);
    detect_pins_and_skewers(board, perspective, &mut patterns);
    detect_hanging_pieces(board, perspective, &mut patterns);
    detect_back_rank_weakness(board, !perspective, &mut patterns);

    let fork_count = patterns
        .iter()
        .filter(|p| matches!(p, TacticalPattern::Fork { .. }))
        .count() as u8;
    let pin_count = patterns
        .iter()
        .filter(|p| matches!(p, TacticalPattern::Pin { .. }))
        .count() as u8;
    let skewer_count = patterns
        .iter()
        .filter(|p| matches!(p, TacticalPattern::Skewer { .. }))
        .count() as u8;
    let discovered_attack_count = patterns
        .iter()
        .filter(|p| matches!(p, TacticalPattern::DiscoveredAttack { .. }))
        .count() as u8;
    let hanging_piece_count = patterns
        .iter()
        .filter(|p| matches!(p, TacticalPattern::HangingPiece { .. }))
        .count() as u8;
    let has_back_rank_weakness = patterns
        .iter()
        .any(|p| matches!(p, TacticalPattern::BackRankWeakness { .. }));

    TacticalAnalysis {
        patterns,
        fork_count,
        pin_count,
        skewer_count,
        discovered_attack_count,
        hanging_piece_count,
        has_back_rank_weakness,
    }
}

/// Detect forks: a piece attacking two or more enemy pieces of higher value,
/// or attacking the king and another piece simultaneously.
fn detect_forks(board: &Board, perspective: Color, patterns: &mut Vec<TacticalPattern>) {
    let enemy = !perspective;
    let enemy_pieces = board.colors(enemy);

    for piece in Piece::ALL {
        let our_pieces = board.pieces(piece) & board.colors(perspective);
        for sq in our_pieces {
            let attacks = piece_attacks(board, sq, piece, perspective) & enemy_pieces;
            if attacks.len() < 2 {
                continue;
            }

            let attacker_val = piece_value(piece);
            let mut targets: Vec<(Square, Piece, u16)> = Vec::new();

            for target_sq in attacks {
                if let Some(target_piece) = board.piece_on(target_sq) {
                    let target_val = piece_value(target_piece);
                    targets.push((target_sq, target_piece, target_val));
                }
            }

            if targets.len() < 2 {
                continue;
            }

            // Fork qualifies if: any target is king, OR any target value > attacker value
            let has_king_target = targets.iter().any(|(_, p, _)| *p == Piece::King);
            let has_higher_value_target = targets.iter().any(|(_, _, v)| *v > attacker_val);

            if has_king_target || has_higher_value_target {
                let target_infos: Vec<SquareInfo> = targets
                    .iter()
                    .map(|(tsq, tp, _)| SquareInfo::new(*tsq, *tp, enemy))
                    .collect();

                patterns.push(TacticalPattern::Fork {
                    attacker: SquareInfo::new(sq, piece, perspective),
                    targets: target_infos,
                });
            }
        }
    }
}

/// Detect pins and skewers: our sliding pieces pinning/skewering enemy pieces.
fn detect_pins_and_skewers(board: &Board, perspective: Color, patterns: &mut Vec<TacticalPattern>) {
    let enemy = !perspective;

    // Check our sliding pieces for pins/skewers against enemy pieces
    for piece in [Piece::Bishop, Piece::Queen, Piece::Rook] {
        let our_sliders = board.pieces(piece) & board.colors(perspective);

        for slider_sq in our_sliders {
            let slider_attacks = piece_attacks(board, slider_sq, piece, perspective);
            let enemy_pieces_bb = board.colors(enemy);

            // For each enemy piece in the slider's attack range
            for front_sq in slider_attacks & enemy_pieces_bb {
                if let Some(front_piece) = board.piece_on(front_sq) {
                    // Check if there's a second enemy piece behind along the same ray
                    if let Some(back_sq) = find_piece_behind(board, slider_sq, front_sq, enemy) {
                        if let Some(back_piece) = board.piece_on(back_sq) {
                            let front_val = piece_value(front_piece);
                            let back_val = piece_value(back_piece);

                            if back_piece == Piece::King || back_val > front_val {
                                // Pin: front piece is less valuable, pinned to back piece
                                patterns.push(TacticalPattern::Pin {
                                    pinner: SquareInfo::new(slider_sq, piece, perspective),
                                    pinned_piece: SquareInfo::new(front_sq, front_piece, enemy),
                                    pinned_to: SquareInfo::new(back_sq, back_piece, enemy),
                                });
                            } else if front_val > back_val {
                                // Skewer: front piece is more valuable
                                patterns.push(TacticalPattern::Skewer {
                                    attacker: SquareInfo::new(slider_sq, piece, perspective),
                                    front_piece: SquareInfo::new(front_sq, front_piece, enemy),
                                    back_piece: SquareInfo::new(back_sq, back_piece, enemy),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Find a piece of `target_color` behind `front_sq` along the ray from `slider_sq` through `front_sq`.
fn find_piece_behind(
    board: &Board,
    slider_sq: Square,
    front_sq: Square,
    target_color: Color,
) -> Option<Square> {
    // Determine ray direction from slider to front
    let slider_rank = slider_sq.rank() as i8;
    let slider_file = slider_sq.file() as i8;
    let front_rank = front_sq.rank() as i8;
    let front_file = front_sq.file() as i8;

    let dr = (front_rank - slider_rank).signum();
    let df = (front_file - slider_file).signum();

    // Walk along the ray from front_sq in the same direction
    let mut r = front_rank + dr;
    let mut f = front_file + df;

    while (0..8).contains(&r) && (0..8).contains(&f) {
        let rank = Rank::try_index(r as usize)?;
        let file = cozy_chess::File::try_index(f as usize)?;
        let sq = Square::new(file, rank);

        if board.occupied().has(sq) {
            // Found an occupied square — check if it's our piece
            if board.colors(target_color).has(sq) {
                return Some(sq);
            }
            // Blocked by another piece
            return None;
        }

        r += dr;
        f += df;
    }

    None
}

/// Detect hanging pieces: enemy pieces attacked but not defended.
fn detect_hanging_pieces(
    board: &Board,
    perspective: Color,
    patterns: &mut Vec<TacticalPattern>,
) {
    let enemy = !perspective;
    let enemy_pieces_bb = board.colors(enemy);

    // Don't count pawns as hanging (too noisy) and don't count the king
    for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
        let targets = board.pieces(piece) & enemy_pieces_bb;
        for sq in targets {
            let our_attackers = attackers_of(board, sq, perspective);
            let their_defenders = attackers_of(board, sq, enemy);

            if our_attackers.len() > 0 && their_defenders.len() == 0 {
                patterns.push(TacticalPattern::HangingPiece {
                    piece: SquareInfo::new(sq, piece, enemy),
                    attacker_count: our_attackers.len() as u8,
                    defender_count: 0,
                });
            }
        }
    }
}

/// Detect back-rank weakness for the given `weak_side`.
fn detect_back_rank_weakness(
    board: &Board,
    weak_side: Color,
    patterns: &mut Vec<TacticalPattern>,
) {
    let strong_side = !weak_side;
    let king_bb = board.pieces(Piece::King) & board.colors(weak_side);
    let king_sq = match king_bb.into_iter().next() {
        Some(sq) => sq,
        None => return,
    };

    // Back rank depends on side
    let back_rank = if weak_side == Color::White {
        Rank::First
    } else {
        Rank::Eighth
    };

    // King must be on back rank
    if king_sq.rank() != back_rank {
        return;
    }

    // Check if all escape squares (king moves) are blocked by own pieces
    let king_moves = cozy_chess::get_king_moves(king_sq);
    let own_pieces = board.colors(weak_side);

    // Escape squares are king moves not on the back rank (moving forward)
    let back_rank_bb = rank_bitboard(back_rank);
    let escape_squares = king_moves & !back_rank_bb;

    // If all escape squares are blocked by own pieces
    if !escape_squares.is_empty() && (escape_squares & !own_pieces).is_empty() {
        // Check if opponent has a rook or queen that could reach the back rank
        let enemy_major = (board.pieces(Piece::Rook) | board.pieces(Piece::Queen))
            & board.colors(strong_side);
        let enemy_attacks = attacked_squares(board, strong_side);

        if enemy_major.len() > 0 && !(enemy_attacks & back_rank_bb).is_empty() {
            patterns.push(TacticalPattern::BackRankWeakness {
                king_square: SquareInfo::new(king_sq, Piece::King, weak_side),
                blocking_rank: back_rank as u8,
            });
        }
    }
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
    fn test_knight_fork() {
        // White knight on d5 forking black queen on c3 and black rook on f6
        let board: Board =
            "4k3/8/5r2/3N4/8/2q5/8/4K3 w - - 0 1".parse().unwrap();
        let analysis = analyze_tactics(&board, Color::White);
        assert!(analysis.fork_count > 0, "Should detect knight fork");
    }

    #[test]
    fn test_pin_detection() {
        // White bishop on a4 pinning black knight on c6 to black king on e8
        let board: Board =
            "4k3/8/2n5/8/B7/8/8/4K3 w - - 0 1".parse().unwrap();
        let analysis = analyze_tactics(&board, Color::White);
        assert!(analysis.pin_count > 0, "Should detect bishop pin");
    }

    #[test]
    fn test_hanging_piece() {
        // Black knight on d5 with no defenders, white bishop attacks it
        let board: Board =
            "4k3/8/8/3n4/8/5B2/8/4K3 w - - 0 1".parse().unwrap();
        let analysis = analyze_tactics(&board, Color::White);
        assert!(
            analysis.hanging_piece_count > 0,
            "Should detect hanging knight"
        );
    }

    #[test]
    fn test_no_tactics_starting_position() {
        let board = Board::default();
        let analysis = analyze_tactics(&board, Color::White);
        // Starting position has no forks, pins, skewers, or hanging pieces
        assert_eq!(analysis.fork_count, 0);
        assert_eq!(analysis.pin_count, 0);
        assert_eq!(analysis.skewer_count, 0);
        assert_eq!(analysis.hanging_piece_count, 0);
    }

    #[test]
    fn test_back_rank_weakness() {
        // White king on g1, pawns on f2,g2,h2 block escape, black has rook on a8
        let board: Board =
            "r3k3/8/8/8/8/8/5PPP/6K1 b - - 0 1".parse().unwrap();
        let analysis = analyze_tactics(&board, Color::Black);
        assert!(
            analysis.has_back_rank_weakness,
            "Should detect back rank weakness"
        );
    }
}
