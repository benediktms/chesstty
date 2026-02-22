use cozy_chess::{BitBoard, File, Piece, Rank, Square};

use super::detector::{TacticalContext, TacticalDetector};
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects back rank weakness: opponent king trapped on back rank with major piece threat.
pub struct BackRankDetector;

impl TacticalDetector for BackRankDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let weak_side = !perspective; // We detect weakness in the OPPONENT

        let king_bb = ctx.after.pieces(Piece::King) & ctx.after.colors(weak_side);
        let king_sq = match king_bb.into_iter().next() {
            Some(sq) => sq,
            None => return vec![],
        };

        // Back rank depends on side
        let back_rank = if weak_side == cozy_chess::Color::White {
            Rank::First
        } else {
            Rank::Eighth
        };

        // King must be on back rank
        if king_sq.rank() != back_rank {
            return vec![];
        }

        // Escape squares: king moves not on the back rank
        let king_moves = cozy_chess::get_king_moves(king_sq);
        let own_pieces = ctx.after.colors(weak_side);
        let back_rank_bb = rank_bitboard(back_rank);
        let escape_squares = king_moves & !back_rank_bb;

        // All escape squares must be blocked by own pieces
        if escape_squares.is_empty() || !(escape_squares & !own_pieces).is_empty() {
            return vec![];
        }

        // Check if we (perspective) have a rook or queen
        let our_major =
            (ctx.after.pieces(Piece::Rook) | ctx.after.pieces(Piece::Queen))
                & ctx.after.colors(perspective);

        if our_major.is_empty() {
            return vec![];
        }

        // Does any of our major pieces attack the back rank?
        let attacks_back_rank = our_major.into_iter().any(|sq| {
            let piece = ctx.after.piece_on(sq).unwrap_or(Piece::Rook);
            let attacks = match piece {
                Piece::Rook => {
                    cozy_chess::get_rook_moves(sq, ctx.after.occupied())
                }
                Piece::Queen => {
                    cozy_chess::get_rook_moves(sq, ctx.after.occupied())
                        | cozy_chess::get_bishop_moves(sq, ctx.after.occupied())
                }
                _ => BitBoard::EMPTY,
            };
            !(attacks & back_rank_bb).is_empty()
        });

        // Only report if we actually have a major piece threatening the back rank
        if !attacks_back_rank {
            return vec![];
        }

        vec![TacticalTag {
            kind: TacticalTagKind::BackRankWeakness,
            attacker: None,
            victims: vec![king_sq.to_string()],
            target_square: Some(king_sq.to_string()),
            confidence: 0.85,
            note: Some(format!(
                "back rank weakness: {:?} king trapped on {:?}",
                weak_side, back_rank
            )),
            evidence: TacticalEvidence {
                lines: vec![],
                threatened_pieces: vec![king_sq.to_string()],
                defended_by: vec![],
            },
        }]
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
    use cozy_chess::{Board, Color};

    use super::*;
    use crate::board_analysis::attack_map::AttackMap;
    use crate::board_analysis::detector::TacticalContext;

    fn ctx_from_after<'a>(
        board: &'a Board,
        attacks: &'a AttackMap,
        perspective: Color,
    ) -> TacticalContext<'a> {
        TacticalContext {
            before: board,
            after: board,
            mv: None,
            side_to_move_before: perspective,
            before_attacks: attacks,
            after_attacks: attacks,
            eval_before: None,
            eval_after: None,
            best_line: None,
        }
    }

    #[test]
    fn detects_white_back_rank_weakness() {
        // White king on g1, pawns on f2/g2/h2 block escape, black has rook on a8
        let board: Board = "r3k3/8/8/8/8/8/5PPP/6K1 b - - 0 1"
            .parse()
            .expect("valid fen");
        let attacks = AttackMap::compute(&board);
        // Black is the attacker (perspective = Black detects weakness in White)
        let ctx = ctx_from_after(&board, &attacks, Color::Black);

        let tags = BackRankDetector.detect(&ctx);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::BackRankWeakness);
        assert_eq!(tags[0].target_square.as_deref(), Some("g1"));
    }

    #[test]
    fn no_weakness_when_king_not_on_back_rank() {
        // White king on e4 (middle of board), black has rook
        let board: Board = "r3k3/8/8/8/4K3/8/5PPP/8 w - - 0 1"
            .parse()
            .expect("valid fen");
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::Black);

        let tags = BackRankDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn no_weakness_when_escape_exists() {
        // White king on g1, only f2 and h2 pawns (g2 is missing — escape square open)
        let board: Board = "r3k3/8/8/8/8/8/5P1P/6K1 b - - 0 1"
            .parse()
            .expect("valid fen");
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::Black);

        let tags = BackRankDetector.detect(&ctx);
        assert!(
            tags.is_empty(),
            "king has g2 as escape square, should not flag weakness"
        );
    }
}
