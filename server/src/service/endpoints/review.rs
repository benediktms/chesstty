//! Post-game review endpoints

use crate::persistence::{AdvancedAnalysisRepository, FinishedGameRepository, ReviewRepository};
use crate::review::types::{is_white_ply, AnalysisScore, MoveClassification, ReviewStatus};
use crate::review::ReviewManager;
use analysis::advanced::types::{
    AdvancedGameAnalysis, AdvancedPositionAnalysis, PsychologicalProfile,
};
use analysis::board_analysis::{
    KingSafetyMetrics, PositionKingSafety, PositionTensionMetrics, TacticalEvidence, TacticalLine,
    TacticalTag, TacticalTagKind,
};
use chess_proto::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ReviewEndpoints<F, R, A>
where
    F: FinishedGameRepository,
    R: ReviewRepository,
    A: AdvancedAnalysisRepository,
{
    review_manager: Arc<ReviewManager<F, R, A>>,
}

impl<F, R, A> ReviewEndpoints<F, R, A>
where
    F: FinishedGameRepository + Send + Sync + 'static,
    R: ReviewRepository + Send + Sync + 'static,
    A: AdvancedAnalysisRepository + Send + Sync + 'static,
{
    pub fn new(review_manager: Arc<ReviewManager<F, R, A>>) -> Self {
        Self { review_manager }
    }

    pub async fn list_finished_games(
        &self,
        _request: Request<ListFinishedGamesRequest>,
    ) -> Result<Response<ListFinishedGamesResponse>, Status> {
        tracing::info!("RPC list_finished_games");

        let games = self
            .review_manager
            .list_finished_games()
            .await
            .map_err(Status::internal)?;

        let mut proto_games = Vec::with_capacity(games.len());
        for g in games {
            // Check review status for this game
            let review_status = self
                .review_manager
                .get_review(&g.game_id)
                .await
                .ok()
                .flatten()
                .map(|r| convert_review_status_type(&r.status) as i32);

            let game_mode = parse_game_mode_string(&g.game_mode, g.human_side.as_deref());

            proto_games.push(FinishedGameInfo {
                game_id: g.game_id,
                result: g.result,
                result_reason: g.result_reason,
                game_mode: Some(game_mode),
                move_count: g.move_count,
                created_at: g.created_at,
                review_status,
            });
        }

        Ok(Response::new(ListFinishedGamesResponse {
            games: proto_games,
        }))
    }

    pub async fn enqueue_review(
        &self,
        request: Request<EnqueueReviewRequest>,
    ) -> Result<Response<EnqueueReviewResponse>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC enqueue_review");

        self.review_manager
            .enqueue(game_id)
            .await
            .map_err(Status::internal)?;

        Ok(Response::new(EnqueueReviewResponse {
            status: Some(ReviewStatusInfo {
                status: ReviewStatusType::ReviewStatusQueued as i32,
                current_ply: None,
                total_plies: None,
                error: None,
            }),
        }))
    }

    pub async fn get_review_status(
        &self,
        request: Request<GetReviewStatusRequest>,
    ) -> Result<Response<GetReviewStatusResponse>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC get_review_status");

        let status = self
            .review_manager
            .get_status(game_id)
            .await
            .map_err(Status::not_found)?;

        Ok(Response::new(GetReviewStatusResponse {
            status: Some(convert_review_status(&status)),
        }))
    }

    pub async fn get_game_review(
        &self,
        request: Request<GetGameReviewRequest>,
    ) -> Result<Response<GetGameReviewResponse>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC get_game_review");

        let review = self
            .review_manager
            .get_review(game_id)
            .await
            .map_err(Status::internal)?
            .ok_or_else(|| Status::not_found(format!("Review not found: {}", game_id)))?;

        Ok(Response::new(GetGameReviewResponse {
            review: Some(convert_game_review_to_proto(&review)),
        }))
    }

    pub async fn export_review_pgn(
        &self,
        request: Request<ExportReviewPgnRequest>,
    ) -> Result<Response<ExportReviewPgnResponse>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC export_review_pgn");

        let review = self
            .review_manager
            .get_review(game_id)
            .await
            .map_err(Status::internal)?
            .ok_or_else(|| Status::not_found(format!("Review not found: {}", game_id)))?;

        let pgn = generate_annotated_pgn(&review);
        Ok(Response::new(ExportReviewPgnResponse { pgn }))
    }

    pub async fn delete_finished_game(
        &self,
        request: Request<DeleteFinishedGameRequest>,
    ) -> Result<Response<Empty>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC delete_finished_game");

        self.review_manager
            .delete_finished_game(game_id)
            .await
            .map_err(Status::internal)?;

        Ok(Response::new(Empty {}))
    }

    pub async fn get_advanced_analysis(
        &self,
        request: Request<GetAdvancedAnalysisRequest>,
    ) -> Result<Response<GetAdvancedAnalysisResponse>, Status> {
        let game_id = &request.get_ref().game_id;
        tracing::info!(game_id = %game_id, "RPC get_advanced_analysis");

        let analysis = self
            .review_manager
            .get_advanced_analysis(game_id)
            .await
            .map_err(Status::internal)?
            .ok_or_else(|| {
                Status::not_found(format!("Advanced analysis not found: {}", game_id))
            })?;

        Ok(Response::new(GetAdvancedAnalysisResponse {
            analysis: Some(convert_advanced_analysis_to_proto(&analysis)),
        }))
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn convert_review_status_type(status: &ReviewStatus) -> ReviewStatusType {
    match status {
        ReviewStatus::Queued => ReviewStatusType::ReviewStatusQueued,
        ReviewStatus::Analyzing { .. } => ReviewStatusType::ReviewStatusAnalyzing,
        ReviewStatus::Complete => ReviewStatusType::ReviewStatusComplete,
        ReviewStatus::Failed { .. } => ReviewStatusType::ReviewStatusFailed,
    }
}

fn convert_review_status(status: &ReviewStatus) -> ReviewStatusInfo {
    match status {
        ReviewStatus::Queued => ReviewStatusInfo {
            status: ReviewStatusType::ReviewStatusQueued as i32,
            current_ply: None,
            total_plies: None,
            error: None,
        },
        ReviewStatus::Analyzing {
            current_ply,
            total_plies,
        } => ReviewStatusInfo {
            status: ReviewStatusType::ReviewStatusAnalyzing as i32,
            current_ply: Some(*current_ply),
            total_plies: Some(*total_plies),
            error: None,
        },
        ReviewStatus::Complete => ReviewStatusInfo {
            status: ReviewStatusType::ReviewStatusComplete as i32,
            current_ply: None,
            total_plies: None,
            error: None,
        },
        ReviewStatus::Failed { error } => ReviewStatusInfo {
            status: ReviewStatusType::ReviewStatusFailed as i32,
            current_ply: None,
            total_plies: None,
            error: Some(error.clone()),
        },
    }
}

fn convert_score_to_proto(score: &AnalysisScore) -> ReviewScore {
    match score {
        AnalysisScore::Centipawns(cp) => ReviewScore {
            score: Some(review_score::Score::Centipawns(*cp)),
        },
        AnalysisScore::Mate(m) => ReviewScore {
            score: Some(review_score::Score::Mate(*m)),
        },
    }
}

fn convert_classification_to_proto(
    classification: &MoveClassification,
) -> chess_proto::MoveClassification {
    match classification {
        MoveClassification::Best => chess_proto::MoveClassification::ClassificationBest,
        MoveClassification::Excellent => chess_proto::MoveClassification::ClassificationExcellent,
        MoveClassification::Good => chess_proto::MoveClassification::ClassificationGood,
        MoveClassification::Inaccuracy => chess_proto::MoveClassification::ClassificationInaccuracy,
        MoveClassification::Mistake => chess_proto::MoveClassification::ClassificationMistake,
        MoveClassification::Blunder => chess_proto::MoveClassification::ClassificationBlunder,
        MoveClassification::Forced => chess_proto::MoveClassification::ClassificationForced,
        MoveClassification::Book => chess_proto::MoveClassification::ClassificationBook,
        MoveClassification::Brilliant => chess_proto::MoveClassification::ClassificationBrilliant,
    }
}

fn convert_game_review_to_proto(
    review: &crate::review::types::GameReview,
) -> chess_proto::GameReviewProto {
    chess_proto::GameReviewProto {
        game_id: review.game_id.clone(),
        status: Some(convert_review_status(&review.status)),
        positions: review
            .positions
            .iter()
            .map(|p| chess_proto::PositionReview {
                ply: p.ply,
                fen: p.fen.clone(),
                played_san: p.played_san.clone(),
                best_move_san: p.best_move_san.clone(),
                best_move_uci: p.best_move_uci.clone(),
                eval_before: Some(convert_score_to_proto(&p.eval_before)),
                eval_after: Some(convert_score_to_proto(&p.eval_after)),
                eval_best: Some(convert_score_to_proto(&p.eval_best)),
                classification: convert_classification_to_proto(&p.classification) as i32,
                cp_loss: p.cp_loss,
                pv: p.pv.clone(),
                depth: p.depth,
                clock_ms: p.clock_ms,
            })
            .collect(),
        white_accuracy: review.white_accuracy,
        black_accuracy: review.black_accuracy,
        total_plies: review.total_plies,
        analyzed_plies: review.analyzed_plies,
        analysis_depth: review.analysis_depth,
        started_at: review.started_at,
        completed_at: review.completed_at,
        winner: review.winner.clone(),
    }
}

fn parse_game_mode_string(mode: &str, human_side: Option<&str>) -> GameModeProto {
    if mode.starts_with("HumanVsEngine") {
        let side = match human_side {
            Some("black") => Some(PlayerSideProto::Black as i32),
            _ => Some(PlayerSideProto::White as i32),
        };
        GameModeProto {
            mode: GameModeType::HumanVsEngine as i32,
            human_side: side,
        }
    } else {
        let mode_type = match mode {
            "EngineVsEngine" => GameModeType::EngineVsEngine,
            "Analysis" => GameModeType::Analysis,
            "Review" => GameModeType::Review,
            _ => GameModeType::HumanVsHuman,
        };
        GameModeProto {
            mode: mode_type as i32,
            human_side: None,
        }
    }
}

// ============================================================================
// Advanced analysis conversion helpers
// ============================================================================

fn convert_advanced_analysis_to_proto(
    analysis: &AdvancedGameAnalysis,
) -> AdvancedGameAnalysisProto {
    AdvancedGameAnalysisProto {
        game_id: analysis.game_id.clone(),
        positions: analysis
            .positions
            .iter()
            .map(convert_advanced_position_to_proto)
            .collect(),
        white_psychology: Some(convert_psychology_to_proto(&analysis.white_psychology)),
        black_psychology: Some(convert_psychology_to_proto(&analysis.black_psychology)),
        pipeline_version: analysis.pipeline_version,
        shallow_depth: analysis.shallow_depth,
        deep_depth: analysis.deep_depth,
        critical_positions_count: analysis.critical_positions_count,
        computed_at: analysis.computed_at,
    }
}

fn convert_advanced_position_to_proto(
    pos: &AdvancedPositionAnalysis,
) -> AdvancedPositionAnalysisProto {
    AdvancedPositionAnalysisProto {
        ply: pos.ply,
        king_safety: Some(convert_king_safety_to_proto(&pos.king_safety)),
        tension: Some(convert_tension_to_proto(&pos.tension)),
        is_critical: pos.is_critical,
        deep_depth: pos.deep_depth,
        tactical_tags_before: pos
            .tactical_tags_before
            .iter()
            .map(convert_tactical_tag_to_proto)
            .collect(),
        tactical_tags_after: pos
            .tactical_tags_after
            .iter()
            .map(convert_tactical_tag_to_proto)
            .collect(),
    }
}

fn convert_tactical_tag_to_proto(tag: &TacticalTag) -> TacticalTagProto {
    TacticalTagProto {
        kind: convert_tactical_tag_kind(&tag.kind) as i32,
        attacker: tag.attacker.clone(),
        victims: tag.victims.clone(),
        target_square: tag.target_square.clone(),
        confidence: tag.confidence,
        note: tag.note.clone(),
        evidence: Some(convert_tactical_evidence_to_proto(&tag.evidence)),
    }
}

fn convert_tactical_tag_kind(kind: &TacticalTagKind) -> TacticalTagKindProto {
    match kind {
        TacticalTagKind::Fork => TacticalTagKindProto::TacticalTagKindFork,
        TacticalTagKind::Pin => TacticalTagKindProto::TacticalTagKindPin,
        TacticalTagKind::Skewer => TacticalTagKindProto::TacticalTagKindSkewer,
        TacticalTagKind::DiscoveredAttack => TacticalTagKindProto::TacticalTagKindDiscoveredAttack,
        TacticalTagKind::DoubleAttack => TacticalTagKindProto::TacticalTagKindDoubleAttack,
        TacticalTagKind::HangingPiece => TacticalTagKindProto::TacticalTagKindHangingPiece,
        TacticalTagKind::Sacrifice => TacticalTagKindProto::TacticalTagKindSacrifice,
        TacticalTagKind::Zwischenzug => TacticalTagKindProto::TacticalTagKindZwischenzug,
        TacticalTagKind::BackRankWeakness => TacticalTagKindProto::TacticalTagKindBackRankWeakness,
        TacticalTagKind::MateThreat => TacticalTagKindProto::TacticalTagKindMateThreat,
    }
}

fn convert_tactical_evidence_to_proto(evidence: &TacticalEvidence) -> TacticalEvidenceProto {
    TacticalEvidenceProto {
        lines: evidence
            .lines
            .iter()
            .map(convert_tactical_line_to_proto)
            .collect(),
        threatened_pieces: evidence.threatened_pieces.clone(),
        defended_by: evidence.defended_by.clone(),
    }
}

fn convert_tactical_line_to_proto(line: &TacticalLine) -> TacticalLineProto {
    TacticalLineProto {
        from: line.from.clone(),
        through: line.through.clone(),
        to: line.to.clone(),
    }
}

fn convert_king_safety_to_proto(safety: &PositionKingSafety) -> PositionKingSafetyProto {
    PositionKingSafetyProto {
        white: Some(convert_king_safety_metrics(&safety.white)),
        black: Some(convert_king_safety_metrics(&safety.black)),
    }
}

fn convert_king_safety_metrics(m: &KingSafetyMetrics) -> KingSafetyMetricsProto {
    KingSafetyMetricsProto {
        color: m.color.to_string(),
        pawn_shield_count: m.pawn_shield_count as u32,
        pawn_shield_max: m.pawn_shield_max as u32,
        open_files_near_king: m.open_files_near_king as u32,
        attacker_count: m.attacker_count as u32,
        attack_weight: m.attack_weight as u32,
        attacked_king_zone_squares: m.attacked_king_zone_squares as u32,
        king_zone_size: m.king_zone_size as u32,
        exposure_score: m.exposure_score,
    }
}

fn convert_tension_to_proto(t: &PositionTensionMetrics) -> PositionTensionMetricsProto {
    PositionTensionMetricsProto {
        mutually_attacked_pairs: t.mutually_attacked_pairs as u32,
        contested_squares: t.contested_squares as u32,
        attacked_but_defended: t.attacked_but_defended as u32,
        forcing_moves: t.forcing_moves as u32,
        checks_available: t.checks_available as u32,
        captures_available: t.captures_available as u32,
        volatility_score: t.volatility_score,
    }
}

fn convert_psychology_to_proto(p: &PsychologicalProfile) -> PsychologicalProfileProto {
    PsychologicalProfileProto {
        color: p.color.to_string(),
        max_consecutive_errors: p.max_consecutive_errors as u32,
        error_streak_start_ply: p.error_streak_start_ply,
        favorable_swings: p.favorable_swings as u32,
        unfavorable_swings: p.unfavorable_swings as u32,
        max_momentum_streak: p.max_momentum_streak as u32,
        blunder_cluster_density: p.blunder_cluster_density as u32,
        blunder_cluster_range_start: p.blunder_cluster_range.map(|(s, _)| s),
        blunder_cluster_range_end: p.blunder_cluster_range.map(|(_, e)| e),
        time_quality_correlation: p.time_quality_correlation,
        avg_blunder_time_ms: p.avg_blunder_time_ms,
        avg_good_move_time_ms: p.avg_good_move_time_ms,
        opening_avg_cp_loss: p.opening_avg_cp_loss,
        middlegame_avg_cp_loss: p.middlegame_avg_cp_loss,
        endgame_avg_cp_loss: p.endgame_avg_cp_loss,
    }
}

fn generate_annotated_pgn(review: &crate::review::types::GameReview) -> String {
    let mut pgn = String::new();

    // PGN headers
    pgn.push_str("[Event \"ChessTTY Game\"]\n");
    pgn.push_str(&format!(
        "[WhiteAccuracy \"{:.1}\"]\n",
        review.white_accuracy.unwrap_or(0.0)
    ));
    pgn.push_str(&format!(
        "[BlackAccuracy \"{:.1}\"]\n",
        review.black_accuracy.unwrap_or(0.0)
    ));
    pgn.push('\n');

    // Moves with annotations
    for pos in review.positions.iter() {
        let is_white = is_white_ply(pos.ply);
        let move_number = (pos.ply as usize + 1) / 2;

        if is_white {
            pgn.push_str(&format!("{}. ", move_number));
        }

        pgn.push_str(&pos.played_san);

        // Add NAG if applicable
        if let Some(nag) = pos.classification.to_nag() {
            pgn.push_str(&format!(" ${}", nag));
        }

        // Add clock annotation if available
        if let Some(ms) = pos.clock_ms {
            let total_secs = ms / 1000;
            let h = total_secs / 3600;
            let m = (total_secs % 3600) / 60;
            let s = total_secs % 60;
            pgn.push_str(&format!(" {{[%clk {}:{:02}:{:02}]}}", h, m, s));
        }

        // Add comment with eval (richer for inaccuracies, mistakes, blunders)
        let comment = match pos.classification {
            MoveClassification::Inaccuracy
            | MoveClassification::Mistake
            | MoveClassification::Blunder => {
                format!(
                    "{{ {}; best: {} ({}cp) }}",
                    pos.eval_before.display(),
                    pos.best_move_san,
                    pos.cp_loss
                )
            }
            _ => format!("{{ {} }}", pos.eval_before.display()),
        };
        pgn.push_str(&format!(" {}", comment));

        pgn.push(' ');
    }

    pgn.trim_end().to_string()
}
