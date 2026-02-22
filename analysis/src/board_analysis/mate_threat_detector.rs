use cozy_chess::{GameStatus, Piece};

use super::detector::{TacticalContext, TacticalDetector};
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects mate threats: positions where the side that just moved has created
/// a situation where the opponent king faces imminent checkmate.
///
/// Detection strategy: check if the opponent (now to move in `ctx.after`) is
/// already in check and has very few legal moves, or if perspective can deliver
/// checkmate in 1 from the current position.
pub struct MateThreatDetector;

impl TacticalDetector for MateThreatDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let opponent = !perspective;

        // Find the opponent king square in the after position.
        let king_sq = {
            let kings = ctx.after.pieces(Piece::King) & ctx.after.colors(opponent);
            match kings.into_iter().next() {
                Some(sq) => sq,
                None => return vec![],
            }
        };

        // Count legal moves available to the opponent in the after position.
        // ctx.after has opponent to move (since perspective just moved).
        // If the side to move in after is actually perspective (unusual ctx setup),
        // we still proceed — checkers() will be for whoever is to move.
        let mut legal_move_count: u32 = 0;
        ctx.after.generate_moves(|moves| {
            legal_move_count += moves.len() as u32;
            false // continue iterating
        });

        let is_in_check = !ctx.after.checkers().is_empty();

        // Determine if this qualifies as a mate threat based on check + mobility.
        if is_in_check && legal_move_count == 0 {
            // Actual checkmate — highest confidence mate threat.
            return vec![TacticalTag {
                kind: TacticalTagKind::MateThreat,
                attacker: None,
                victims: vec![king_sq.to_string()],
                target_square: Some(king_sq.to_string()),
                confidence: 1.0,
                note: Some(format!(
                    "mate threat: {:?} king checkmated (0 legal moves)",
                    opponent
                )),
                evidence: TacticalEvidence::default(),
            }];
        }

        if is_in_check && legal_move_count <= 2 {
            // In check with very few escape moves — near-mate threat.
            return vec![TacticalTag {
                kind: TacticalTagKind::MateThreat,
                attacker: None,
                victims: vec![king_sq.to_string()],
                target_square: Some(king_sq.to_string()),
                confidence: 0.9,
                note: Some(format!(
                    "mate threat: {:?} king has limited escape ({} legal moves)",
                    opponent, legal_move_count
                )),
                evidence: TacticalEvidence::default(),
            }];
        }

        // Non-check case: check if perspective can deliver mate-in-1 from the after position.
        // We do this by generating moves for perspective from a hypothetical board where it's
        // their turn. We use the after board but need perspective to move.
        // Only feasible if the after board has perspective to move (some ctx setups use before=after).
        let after_side_to_move = ctx.after.side_to_move();
        if after_side_to_move == perspective {
            // Perspective can move — check for mate-in-1 directly.
            let mut found_mate = false;
            ctx.after.generate_moves(|moves| {
                for mv in moves {
                    let mut test_board = ctx.after.clone();
                    test_board.play_unchecked(mv);
                    if test_board.status() == GameStatus::Won {
                        found_mate = true;
                        return true; // stop early
                    }
                }
                false
            });

            if found_mate {
                return vec![TacticalTag {
                    kind: TacticalTagKind::MateThreat,
                    attacker: None,
                    victims: vec![king_sq.to_string()],
                    target_square: Some(king_sq.to_string()),
                    confidence: 0.95,
                    note: Some(format!(
                        "mate threat: {:?} king faces mate in 1",
                        opponent
                    )),
                    evidence: TacticalEvidence::default(),
                }];
            }
        }

        vec![]
    }
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
    fn no_mate_threat_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = MateThreatDetector.detect(&ctx);
        assert!(tags.is_empty());
    }

    #[test]
    fn detects_checkmate_position() {
        // Black king g8 is checkmated: Ra8 controls entire 8th rank,
        // pawns f7/g7/h7 block all escape squares. Black to move, 0 legal moves.
        // perspective = White (just delivered the mate).
        let board: Board = "R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = MateThreatDetector.detect(&ctx);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::MateThreat);
        assert_eq!(tags[0].confidence, 1.0);
        assert_eq!(tags[0].victims, vec!["g8"]);
    }

    #[test]
    fn detects_near_mate_pressure_in_check_few_moves() {
        // Black king on h8 in check from White queen on g6, very few escape squares.
        // White rook on h1 covers the h-file. Black king is trapped.
        // FEN: Black king h8, White queen g6, White rook h1, White king g1.
        // After Qg7+, Black has only Qxg6 or Kh7 - but let's set up a direct check position.
        // Black king on h8, White queen on h7 gives check. Black has 0 legal moves (Kg8 blocked by Qh7? no).
        // Simpler: use a known smothered check position.
        // "6k1/5Q2/8/8/8/8/8/6K1 b - - 0 1": Black king g8, White queen f7.
        // Black is in check (Qf7 does NOT check g8... Qf7 attacks g8? No, f7 to g8 is diagonal. Yes!)
        // Black king on g8, Qf7: check. Black legal moves: Kh8 (if not covered), Kh7 (if not covered).
        // Qf7 covers g8,h7,g6,e8,e6... Kh8 is legal. So not a 0-move position.
        //
        // Use a well-known back-rank mate threat:
        // "6k1/5ppp/8/8/8/8/8/3Q2K1 b - - 0 1"
        // Black king g8 with pawns on f7,g7,h7. White Qd1. Not in check.
        //
        // Easiest: construct an actual checkmate position and verify confidence=1.0
        // "6k1/5Qpp/8/8/8/8/8/6K1 b - - 0 1": Qf7 → does it mate? Kh8 is free.
        //
        // Use "R6k/8/7K/8/8/8/8/8 b - - 0 1": Black king h8, White Ra8 would be mate but it's black to move.
        // Let's place it so Black IS in check: "R6k/8/7K/8/8/8/8/8 b - - 0 1"
        // Ra8 is on a8? No — Ra8 not shown. Let's try: "7k/R7/7K/8/8/8/8/8 b - - 0 1"
        // White Ra7: does it check h8? No, Ra7 attacks along rank 7 and file a.
        // "6Rk/8/7K/8/8/8/8/8 b - - 0 1": White Rg8 checks Black king h8. Kh7 is the only move (if K is on h6 that blocks). Wk h6 means Kh7 allowed? Rg8+ Kh7, now 1 legal move.
        let board: Board = "6Rk/8/7K/8/8/8/8/8 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        // Black king is in check from Rg8. perspective=White (just moved to give check).
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = MateThreatDetector.detect(&ctx);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::MateThreat);
        // In check, should have limited moves → confidence 0.9
        assert!(tags[0].confidence >= 0.9);
        assert!(tags[0].note.as_ref().unwrap().contains("limited escape") ||
                tags[0].note.as_ref().unwrap().contains("checkmated"));
    }

    #[test]
    fn detects_mate_in_one_threat() {
        // Classic back-rank mate: White rook on a1 can play Ra8#.
        // Black king on g8 with pawns f7/g7/h7 blocking all escape.
        // Rook checks along the entire 8th rank — king can't capture (too far).
        let board: Board = "6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        // perspective = White, after board has White to move → mate-in-1 branch
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = MateThreatDetector.detect(&ctx);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TacticalTagKind::MateThreat);
        assert!(tags[0].confidence >= 0.9);
        assert_eq!(tags[0].victims, vec!["g8"]);
    }

    #[test]
    fn no_mate_threat_open_position() {
        // Open position with many moves available — no mate threat.
        // Both kings far apart, no immediate danger.
        let board: Board = "4k3/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = MateThreatDetector.detect(&ctx);
        assert!(tags.is_empty());
    }
}
