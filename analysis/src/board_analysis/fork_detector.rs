use cozy_chess::Piece;

use super::detector::{TacticalContext, TacticalDetector};
use super::helpers::{piece_attacks, piece_value};
use super::tactical_types::{TacticalEvidence, TacticalTag, TacticalTagKind};

/// Detects forks: a single piece attacking two or more enemy pieces simultaneously,
/// where at least one target is the king OR at least one target is worth more than the attacker.
pub struct ForkDetector;

impl TacticalDetector for ForkDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;
        let enemy_pieces_bb = ctx.after.colors(enemy);

        let mut tags = Vec::new();

        for piece in Piece::ALL {
            let our_pieces = ctx.after.pieces(piece) & ctx.after.colors(perspective);
            for sq in our_pieces {
                let attacks = piece_attacks(ctx.after, sq, piece, perspective) & enemy_pieces_bb;
                if attacks.len() < 2 {
                    continue;
                }

                let attacker_val = piece_value(piece);
                let mut targets: Vec<(cozy_chess::Square, Piece, u16)> = Vec::new();

                for target_sq in attacks {
                    if let Some(target_piece) = ctx.after.piece_on(target_sq) {
                        let target_val = piece_value(target_piece);
                        targets.push((target_sq, target_piece, target_val));
                    }
                }

                if targets.len() < 2 {
                    continue;
                }

                let has_king_target = targets.iter().any(|(_, p, _)| *p == Piece::King);
                let has_higher_value_target = targets.iter().any(|(_, _, v)| *v > attacker_val);

                if has_king_target || has_higher_value_target {
                    let victims: Vec<String> =
                        targets.iter().map(|(tsq, _, _)| tsq.to_string()).collect();
                    let confidence = if has_king_target { 0.95 } else { 0.85 };

                    tags.push(TacticalTag {
                        kind: TacticalTagKind::Fork,
                        attacker: Some(sq.to_string()),
                        victims: victims.clone(),
                        target_square: None,
                        confidence,
                        note: Some(format!(
                            "fork: {} on {} attacks {} pieces",
                            piece,
                            sq,
                            targets.len()
                        )),
                        evidence: TacticalEvidence {
                            lines: vec![],
                            threatened_pieces: victims,
                            defended_by: vec![],
                        },
                    });
                }
            }
        }

        tags
    }
}

/// Detects double attacks: any piece of the perspective side that attacks two or more
/// enemy pieces where the enemy piece has fewer defenders than attackers from our side.
pub struct DoubleAttackDetector;

impl TacticalDetector for DoubleAttackDetector {
    fn detect(&self, ctx: &TacticalContext) -> Vec<TacticalTag> {
        let perspective = ctx.side_to_move_before;
        let enemy = !perspective;
        let enemy_pieces_bb = ctx.after.colors(enemy);

        let mut tags = Vec::new();

        for piece in Piece::ALL {
            let our_pieces = ctx.after.pieces(piece) & ctx.after.colors(perspective);
            for sq in our_pieces {
                let attacks = piece_attacks(ctx.after, sq, piece, perspective) & enemy_pieces_bb;

                let mut vulnerable_targets: Vec<String> = Vec::new();

                for target_sq in attacks {
                    let our_attackers =
                        ctx.after_attacks.attackers_of(target_sq, perspective).len();
                    let their_defenders = ctx.after_attacks.attackers_of(target_sq, enemy).len();

                    if our_attackers > their_defenders {
                        vulnerable_targets.push(target_sq.to_string());
                    }
                }

                if vulnerable_targets.len() >= 2 {
                    tags.push(TacticalTag {
                        kind: TacticalTagKind::DoubleAttack,
                        attacker: Some(sq.to_string()),
                        victims: vulnerable_targets.clone(),
                        target_square: None,
                        confidence: 0.7,
                        note: Some(format!(
                            "double attack: {} on {} threatens {} under-defended pieces",
                            piece,
                            sq,
                            vulnerable_targets.len()
                        )),
                        evidence: TacticalEvidence {
                            lines: vec![],
                            threatened_pieces: vulnerable_targets,
                            defended_by: vec![],
                        },
                    });
                }
            }
        }

        tags
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
    fn detects_knight_fork_king_and_queen() {
        // White knight on f6 forks black king on e8 and black rook on d7.
        // Knight on f6 attacks: d5, d7, e4, e8, g4, g8, h5, h7 → hits e8 (king) and d7 (rook).
        // FEN uses "b" to move: white just played Nf6+, so it is now black's turn.
        // side_to_move_before = White (the forking side).
        let board: Board = "4k3/3r4/5N2/8/8/8/8/4K3 b - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = ForkDetector.detect(&ctx);

        assert!(!tags.is_empty(), "should detect fork");
        let fork = tags
            .iter()
            .find(|t| t.attacker.as_deref() == Some("f6"))
            .expect("fork by Nf6");
        assert_eq!(fork.kind, TacticalTagKind::Fork);
        assert!(fork.victims.contains(&"e8".to_string()));
        assert!(fork.victims.contains(&"d7".to_string()));
        assert_eq!(fork.confidence, 0.95, "king target → confidence 0.95");
    }

    #[test]
    fn detects_knight_fork_rook_and_bishop() {
        // White knight on d5 forking black rook on c7 and black bishop on f6
        // Knight on d5 attacks: b4, b6, c3, c7, e3, e7, f4, f6
        // Rook on c7 (value 500 > knight 320) ✓, bishop on f6 (value 330 > 320) ✓
        let board: Board = "4k3/2r5/5b2/3N4/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = ForkDetector.detect(&ctx);

        assert!(!tags.is_empty(), "should detect fork of rook and bishop");
        let fork = tags
            .iter()
            .find(|t| t.attacker.as_deref() == Some("d5"))
            .unwrap();
        assert_eq!(fork.kind, TacticalTagKind::Fork);
        assert!(fork.victims.contains(&"c7".to_string()));
        assert!(fork.victims.contains(&"f6".to_string()));
        // No king target, but targets (rook/bishop) are both worth more than knight
        assert_eq!(fork.confidence, 0.85);
    }

    #[test]
    fn no_fork_single_target() {
        // White knight on d5 with only one enemy piece in range
        // Knight on d5 attacks b4,b6,c3,c7,e3,e7,f4,f6 — place only one enemy on c7
        let board: Board = "4k3/2r5/8/3N4/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = ForkDetector.detect(&ctx);

        // Only one target (rook on c7) → no fork
        assert!(
            tags.is_empty(),
            "single target should not produce a fork tag"
        );
    }

    #[test]
    fn no_forks_starting_position() {
        let board = Board::default();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = ForkDetector.detect(&ctx);
        assert!(tags.is_empty(), "starting position has no forks");
    }

    #[test]
    fn detects_double_attack() {
        // White rook on a5 attacks the entire a-file and a-rank.
        // Black bishop on a7 (no defender) and black knight on a2 (no defender).
        // White rook is the only attacker of each → attacker_count(1) > defender_count(0).
        // FEN: rook on a5 (R), black bishop on a7 (b), black knight on a2 (n),
        //      black king on h8, white king on h1 (away from the action, not defending a-file targets).
        let board: Board = "7k/b7/8/R7/8/8/n7/7K w - - 0 1".parse().unwrap();
        let attacks = AttackMap::compute(&board);
        let ctx = ctx_from_after(&board, &attacks, Color::White);

        let tags = DoubleAttackDetector.detect(&ctx);

        assert!(
            !tags.is_empty(),
            "should detect double attack on two undefended pieces"
        );
        let da = tags
            .iter()
            .find(|t| t.attacker.as_deref() == Some("a5"))
            .expect("double attack by Ra5");
        assert_eq!(da.kind, TacticalTagKind::DoubleAttack);
        assert_eq!(da.confidence, 0.7);
        assert!(da.victims.len() >= 2);
    }
}
