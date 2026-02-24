use std::collections::HashSet;
use std::sync::Arc;

use analysis::AnalysisConfig;
use engine::{EngineCommand, EngineEvent, GoParams, StockfishConfig, StockfishEngine};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::persistence::{AdvancedAnalysisRepository, Persistence, ReviewRepository};

use super::advanced::compute_advanced_analysis;
use super::types::*;

/// A long-lived worker task. Receives jobs from the shared channel,
/// processes them one at a time.
pub async fn run_review_worker<D: Persistence>(
    worker_id: usize,
    job_rx: Arc<Mutex<mpsc::Receiver<ReviewJob>>>,
    store: Arc<D::Reviews>,
    advanced_store: Arc<D::Advanced>,
    enqueued: Arc<RwLock<HashSet<String>>>,
    analysis_depth: u32,
    analysis_config: AnalysisConfig,
) {
    tracing::info!(worker_id, "Review worker started");

    loop {
        // Wait for next job (only one worker picks up each job)
        tracing::debug!(worker_id, "Waiting for next job");
        let job = {
            let mut rx = job_rx.lock().await;
            match rx.recv().await {
                Some(job) => job,
                None => {
                    tracing::info!(worker_id, "Job channel closed, worker exiting");
                    break;
                }
            }
        };

        tracing::info!(worker_id, game_id = %job.game_id, plies = job.game_data.moves.len(), "Starting review analysis");

        let result = analyze_game::<D>(
            worker_id,
            &job,
            store.as_ref(),
            advanced_store.as_ref(),
            analysis_depth,
            &analysis_config,
        )
        .await;

        match result {
            Ok(()) => {
                tracing::info!(worker_id, game_id = %job.game_id, "Review analysis complete");
            }
            Err(e) => {
                tracing::error!(worker_id, game_id = %job.game_id, "Review analysis failed: {}", e);
                let failed_review = GameReview {
                    game_id: job.game_id.clone(),
                    status: ReviewStatus::Failed { error: e },
                    positions: vec![],
                    white_accuracy: None,
                    black_accuracy: None,
                    total_plies: job.game_data.move_count,
                    analyzed_plies: 0,
                    analysis_depth,
                    started_at: None,
                    completed_at: None,
                    winner: None,
                };
                let _ = store.save_review(&failed_review).await;
            }
        }

        // Remove from enqueued set
        enqueued.write().await.remove(&job.game_id);
    }
}

/// Analyze all positions in a finished game.
///
/// Pipeline:
///   Phase 1 — Engine analysis of each position (at configured depth)
///   Phase 2+4 — Board geometry metrics + psychological profiling (via analysis crate)
async fn analyze_game<D: Persistence>(
    worker_id: usize,
    job: &ReviewJob,
    store: &D::Reviews,
    advanced_store: &D::Advanced,
    analysis_depth: u32,
    analysis_config: &AnalysisConfig,
) -> Result<(), String> {
    let game = &job.game_data;
    let total_plies = game.moves.len() as u32;

    // Check for partial review (crash recovery)
    let mut review = match store.load_review(&job.game_id).await {
        Ok(Some(existing)) if !existing.positions.is_empty() => {
            tracing::info!(
                worker_id,
                game_id = %job.game_id,
                "Resuming from ply {}",
                existing.analyzed_plies
            );
            existing
        }
        _ => GameReview {
            game_id: job.game_id.clone(),
            status: ReviewStatus::Analyzing {
                current_ply: 0,
                total_plies,
            },
            positions: Vec::with_capacity(total_plies as usize),
            white_accuracy: None,
            black_accuracy: None,
            total_plies,
            analyzed_plies: 0,
            analysis_depth,
            started_at: Some(crate::persistence::now_timestamp()),
            completed_at: None,
            winner: None,
        },
    };

    // =====================================================================
    // Phase 1: Engine analysis of each position
    // =====================================================================
    tracing::info!(worker_id, game_id = %job.game_id, "Spawning Stockfish for analysis");
    let sf_config = StockfishConfig {
        skill_level: None, // Full strength for analysis
        threads: Some(1),  // One thread per worker to bound resources
        hash_mb: Some(64), // Moderate hash for analysis
        label: Some(format!("review-worker-{}", worker_id)),
    };
    let mut engine = StockfishEngine::spawn_with_config(sf_config)
        .await
        .map_err(|e| format!("Failed to spawn engine: {}", e))?;

    tracing::info!(worker_id, game_id = %job.game_id, "Stockfish spawned, beginning ply analysis");

    let start_ply = review.analyzed_plies as usize;

    for (i, move_record) in game.moves.iter().enumerate().skip(start_ply) {
        let ply = (i as u32) + 1; // 1-indexed: ply 1 = first move
        let is_white_move = i % 2 == 0;
        let side = if is_white_move { "W" } else { "B" };

        tracing::info!(
            worker_id,
            game_id = %job.game_id,
            ply = ply,
            total = total_plies,
            side = side,
            san = %move_record.san,
            "Analyzing ply {}/{}",
            ply + 1,
            total_plies
        );

        // The FEN *before* this move
        let fen_before = if i == 0 {
            game.start_fen.clone()
        } else {
            game.moves[i - 1].fen_after.clone()
        };

        // 1. Evaluate the position before the move to find the best move and eval
        let (best_eval, best_move_uci, pv) =
            evaluate_position(&mut engine, &fen_before, analysis_depth).await?;

        // 2. Evaluate the position after the played move
        //    Skip engine call for terminal positions (checkmate/stalemate) —
        //    Stockfish responds with `bestmove (none)` which our parser can't handle.
        let fen_after = &move_record.fen_after;
        let played_eval = if is_terminal_position(fen_after) {
            // Terminal position: infer eval from game status.
            // From the side-to-move's perspective in a terminal position:
            // - Checkmate: the side to move is mated → Mate(0) (being mated right now)
            // - Stalemate: draw → Centipawns(0)
            if is_checkmate(fen_after) {
                AnalysisScore::Mate(0) // side-to-move is checkmated
            } else {
                AnalysisScore::Centipawns(0) // stalemate = draw
            }
        } else {
            let (eval, _, _) = evaluate_position(&mut engine, fen_after, analysis_depth).await?;
            eval
        };

        // Compute cp_loss from the moving side's perspective:
        // best_eval is from the moving side's perspective (before the move).
        // played_eval is from the *opponent's* perspective (after the move, it's their turn).
        // So we negate played_eval to get it from the moving side's perspective.
        let best_cp = best_eval.to_cp();
        let played_cp = played_eval.negate().to_cp();
        let cp_loss = (best_cp - played_cp).max(0);

        // Check if move was forced (only one legal move)
        let is_forced = check_forced_move(&fen_before);

        let classification = MoveClassification::from_cp_loss(cp_loss, is_forced);

        tracing::debug!(
            worker_id,
            game_id = %job.game_id,
            ply = ply,
            san = %move_record.san,
            best = %best_move_uci,
            cp_loss = cp_loss,
            classification = ?classification,
            "Ply analyzed"
        );

        // Store evals normalized to White's perspective for consistency
        let eval_before_white = if is_white_move {
            best_eval.clone()
        } else {
            best_eval.negate()
        };
        let eval_after_white = if is_white_move {
            played_eval.negate() // After white's move, eval is from black's perspective
        } else {
            played_eval.clone() // After black's move, eval is from white's perspective
        };

        // Convert best move from UCI to SAN using the board position
        let best_move_san = uci_to_san(&fen_before, &best_move_uci);

        let position_review = PositionReview {
            ply,
            fen: move_record.fen_after.clone(),
            played_san: move_record.san.clone(),
            best_move_san,
            best_move_uci,
            eval_before: eval_before_white.clone(),
            eval_after: eval_after_white,
            eval_best: eval_before_white,
            classification,
            cp_loss,
            pv,
            depth: analysis_depth,
            clock_ms: move_record.clock_ms,
        };

        review.positions.push(position_review);
        review.analyzed_plies = ply;
        review.status = ReviewStatus::Analyzing {
            current_ply: ply,
            total_plies,
        };

        // Persist partial results after each ply (crash recovery)
        store
            .save_review(&review)
            .await
            .map_err(|e| format!("Failed to save partial review: {}", e))?;
    }

    // Compute accuracy scores
    review.white_accuracy = Some(compute_accuracy(&review.positions, true));
    review.black_accuracy = Some(compute_accuracy(&review.positions, false));

    // Set winner from game result
    review.winner = match job.game_data.result.as_str() {
        "WhiteWins" => Some("White".to_string()),
        "BlackWins" => Some("Black".to_string()),
        "Draw" => Some("Draw".to_string()),
        _ => None,
    };

    review.status = ReviewStatus::Complete;
    review.completed_at = Some(crate::persistence::now_timestamp());

    tracing::info!(
        worker_id,
        game_id = %job.game_id,
        white_accuracy = ?review.white_accuracy,
        black_accuracy = ?review.black_accuracy,
        plies = review.analyzed_plies,
        "Analysis complete, saving results"
    );

    store
        .save_review(&review)
        .await
        .map_err(|e| format!("Failed to save completed review: {}", e))?;

    // =====================================================================
    // Phase 2+4: Advanced analysis (board geometry + psychological profiling)
    // =====================================================================
    if analysis_config.compute_advanced {
        tracing::info!(
            worker_id,
            game_id = %job.game_id,
            "Computing advanced analysis"
        );

        let advanced = compute_advanced_analysis(
            &review,
            analysis_config,
            crate::persistence::now_timestamp(),
        );

        tracing::info!(
            worker_id,
            game_id = %job.game_id,
            critical_positions = advanced.critical_positions_count,
            "Advanced analysis complete, saving"
        );

        advanced_store
            .save_analysis(&advanced)
            .await
            .map_err(|e| format!("Failed to save advanced analysis: {}", e))?;
    }

    // Shutdown engine
    tracing::debug!(worker_id, game_id = %job.game_id, "Shutting down Stockfish");
    engine.shutdown().await;

    Ok(())
}

/// Run engine analysis on a position and return (score, best_move_uci, pv).
async fn evaluate_position(
    engine: &mut StockfishEngine,
    fen: &str,
    depth: u32,
) -> Result<(AnalysisScore, String, Vec<String>), String> {
    engine
        .send_command(EngineCommand::SetPosition {
            fen: fen.to_string(),
            moves: vec![],
        })
        .await
        .map_err(|e| e.to_string())?;

    engine
        .send_command(EngineCommand::Go(GoParams {
            depth: Some(depth as u8),
            movetime: None,
            infinite: false,
        }))
        .await
        .map_err(|e| e.to_string())?;

    // Collect engine output until BestMove
    let mut last_score = AnalysisScore::Centipawns(0);
    let mut pv_moves = vec![];

    loop {
        match engine.recv_event().await {
            Some(EngineEvent::Info(info)) => {
                if let Some(score) = info.score {
                    last_score = match score {
                        engine::Score::Centipawns(cp) => AnalysisScore::Centipawns(cp),
                        engine::Score::Mate(m) => AnalysisScore::Mate(m as i32),
                    };
                }
                if !info.pv.is_empty() {
                    pv_moves = info.pv.iter().map(|m| chess::format_uci_move(*m)).collect();
                }
            }
            Some(EngineEvent::BestMove(mv)) => {
                let best_uci = chess::format_uci_move(mv);
                return Ok((last_score, best_uci, pv_moves));
            }
            Some(EngineEvent::Error(e)) => {
                return Err(format!("Engine error during analysis: {}", e));
            }
            Some(_) => continue,
            None => {
                return Err("Engine channel closed during analysis".to_string());
            }
        }
    }
}

/// Check if there is only one legal move in a position (forced move).
fn check_forced_move(fen: &str) -> bool {
    if let Ok(board) = fen.parse::<cozy_chess::Board>() {
        let mut count = 0;
        board.generate_moves(|moves| {
            count += moves.len();
            count <= 1
        });
        count == 1
    } else {
        false
    }
}

/// Check if a position has no legal moves (checkmate or stalemate).
fn is_terminal_position(fen: &str) -> bool {
    if let Ok(board) = fen.parse::<cozy_chess::Board>() {
        board.status() != cozy_chess::GameStatus::Ongoing
    } else {
        false
    }
}

/// Convert a UCI move string to SAN given a FEN position.
/// Falls back to the UCI string if parsing fails.
fn uci_to_san(fen: &str, uci: &str) -> String {
    let board = match fen.parse::<cozy_chess::Board>() {
        Ok(b) => b,
        Err(_) => return uci.to_string(),
    };
    let mv = match engine::uci::parser::parse_uci_move(uci) {
        Ok(m) => m,
        Err(_) => return uci.to_string(),
    };
    chess::format_move_as_san(&board, mv)
}

/// Check if a position is checkmate (as opposed to stalemate).
fn is_checkmate(fen: &str) -> bool {
    if let Ok(board) = fen.parse::<cozy_chess::Board>() {
        board.status() == cozy_chess::GameStatus::Won
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn test_uci_to_san_pawn_push() {
        assert_eq!(uci_to_san(START_FEN, "e2e4"), "e4");
    }

    #[test]
    fn test_uci_to_san_knight() {
        assert_eq!(uci_to_san(START_FEN, "g1f3"), "Nf3");
    }

    #[test]
    fn test_uci_to_san_capture() {
        let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";
        assert_eq!(uci_to_san(fen, "e4d5"), "exd5");
    }

    #[test]
    fn test_uci_to_san_castling_kingside() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQK2R w KQkq - 0 1";
        assert_eq!(uci_to_san(fen, "e1h1"), "O-O");
    }

    #[test]
    fn test_uci_to_san_castling_queenside() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R3KBNR w KQkq - 0 1";
        assert_eq!(uci_to_san(fen, "e1a1"), "O-O-O");
    }

    #[test]
    fn test_uci_to_san_promotion() {
        let fen = "8/P7/8/8/8/8/8/4K2k w - - 0 1";
        assert_eq!(uci_to_san(fen, "a7a8q"), "a8=Q");
    }

    #[test]
    fn test_uci_to_san_invalid_fen_falls_back() {
        assert_eq!(uci_to_san("not a fen", "e2e4"), "e2e4");
    }

    #[test]
    fn test_uci_to_san_invalid_uci_falls_back() {
        assert_eq!(uci_to_san(START_FEN, "zz"), "zz");
    }

    #[test]
    fn test_is_terminal_checkmate() {
        // Fool's mate final position
        let fen = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
        assert!(is_terminal_position(fen));
        assert!(is_checkmate(fen));
    }

    #[test]
    fn test_is_terminal_stalemate() {
        // Stalemate position: black king on a8, white queen on b6, white king on c8
        let fen = "k7/8/1Q6/8/8/8/8/2K5 b - - 0 1";
        assert!(is_terminal_position(fen));
        assert!(!is_checkmate(fen));
    }

    #[test]
    fn test_not_terminal_ongoing() {
        assert!(!is_terminal_position(START_FEN));
    }
}
