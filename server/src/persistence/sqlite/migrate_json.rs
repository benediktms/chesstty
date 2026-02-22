use std::path::Path;

use analysis::{AdvancedGameAnalysis, GameReview};
use sqlx::{SqlitePool, Transaction};
use tracing::info;

use super::helpers::{encode_classification, encode_score, encode_status, normalize_game_mode};
use crate::persistence::{
    FinishedGameData, JsonStore, PersistenceError, SavedPositionData, Storable,
    SuspendedSessionData, now_timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub skipped: bool,
    pub sessions: u64,
    pub positions: u64,
    pub finished_games: u64,
    pub reviews: u64,
    pub advanced_analyses: u64,
}

impl MigrationReport {
    fn has_data(&self) -> bool {
        self.sessions > 0
            || self.positions > 0
            || self.finished_games > 0
            || self.reviews > 0
            || self.advanced_analyses > 0
    }
}

pub async fn migrate_json_to_sqlite(
    pool: &SqlitePool,
    data_dir: &Path,
) -> Result<MigrationReport, PersistenceError> {
    info!(data_dir = %data_dir.display(), "Starting JSON to SQLite migration");

    let existing = sqlite_counts(pool).await?;
    if existing.has_data() {
        info!(
            sessions = existing.sessions,
            positions = existing.positions,
            finished_games = existing.finished_games,
            reviews = existing.reviews,
            advanced_analyses = existing.advanced_analyses,
            "SQLite already contains data, skipping JSON migration"
        );
        return Ok(MigrationReport {
            skipped: true,
            ..existing
        });
    }

    let sessions = load_all_with_doubled_fallback::<SuspendedSessionData>(data_dir, "sessions")?;
    let positions = load_all_with_doubled_fallback::<SavedPositionData>(data_dir, "positions")?;
    let finished_games =
        load_all_with_doubled_fallback::<FinishedGameData>(data_dir, "finished_games")?;
    let reviews = load_all_with_doubled_fallback::<GameReview>(data_dir, "reviews")?;
    let advanced_analyses =
        load_all_with_doubled_fallback::<AdvancedGameAnalysis>(data_dir, "advanced_reviews")?;

    info!(
        sessions = sessions.len(),
        positions = positions.len(),
        finished_games = finished_games.len(),
        reviews = reviews.len(),
        advanced_analyses = advanced_analyses.len(),
        "Loaded JSON records for migration"
    );

    let mut tx = pool.begin().await?;

    insert_sessions(&mut tx, &sessions).await?;
    insert_positions(&mut tx, &positions).await?;
    insert_finished_games(&mut tx, &finished_games).await?;
    insert_reviews(&mut tx, &reviews).await?;
    insert_advanced_analyses(&mut tx, &advanced_analyses).await?;

    tx.commit().await?;

    let report = MigrationReport {
        skipped: false,
        sessions: sessions.len() as u64,
        positions: positions.len() as u64,
        finished_games: finished_games.len() as u64,
        reviews: reviews.len() as u64,
        advanced_analyses: advanced_analyses.len() as u64,
    };

    info!(
        sessions = report.sessions,
        positions = report.positions,
        finished_games = report.finished_games,
        reviews = report.reviews,
        advanced_analyses = report.advanced_analyses,
        "JSON to SQLite migration completed"
    );

    Ok(report)
}

fn load_all_with_doubled_fallback<T: Storable>(
    data_dir: &Path,
    subdir: &str,
) -> Result<Vec<T>, PersistenceError> {
    let doubled_path = data_dir.join(subdir).join(subdir);
    let doubled_store = JsonStore::<T>::new(doubled_path.clone());
    let doubled_items = doubled_store.load_all()?;
    if !doubled_items.is_empty() {
        info!(
            path = %doubled_path.display(),
            count = doubled_items.len(),
            "Loaded JSON records from doubled path"
        );
        return Ok(doubled_items);
    }

    let normal_path = data_dir.join(subdir);
    let normal_store = JsonStore::<T>::new(normal_path.clone());
    let normal_items = normal_store.load_all()?;
    if !normal_items.is_empty() {
        info!(
            path = %normal_path.display(),
            count = normal_items.len(),
            "Loaded JSON records from normal path"
        );
    }

    Ok(normal_items)
}

async fn sqlite_counts(pool: &SqlitePool) -> Result<MigrationReport, PersistenceError> {
    Ok(MigrationReport {
        skipped: false,
        sessions: table_count(pool, "suspended_sessions").await?,
        positions: table_count(pool, "saved_positions").await?,
        finished_games: table_count(pool, "finished_games").await?,
        reviews: table_count(pool, "game_reviews").await?,
        advanced_analyses: table_count(pool, "advanced_game_analyses").await?,
    })
}

async fn table_count(pool: &SqlitePool, table: &str) -> Result<u64, PersistenceError> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let row: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
    Ok(row.0 as u64)
}

async fn insert_sessions(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    sessions: &[SuspendedSessionData],
) -> Result<(), PersistenceError> {
    for data in sessions {
        let game_mode = normalize_game_mode(&data.game_mode);
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO suspended_sessions
                (suspended_id, fen, side_to_move, move_count, game_mode,
                 human_side, skill_level, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&data.suspended_id)
        .bind(&data.fen)
        .bind(&data.side_to_move)
        .bind(data.move_count as i64)
        .bind(game_mode)
        .bind(&data.human_side)
        .bind(data.skill_level as i64)
        .bind(data.created_at as i64)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_positions(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    positions: &[SavedPositionData],
) -> Result<(), PersistenceError> {
    for data in positions {
        sqlx::query(
            "INSERT OR REPLACE INTO saved_positions \
             (position_id, name, fen, is_default, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&data.position_id)
        .bind(&data.name)
        .bind(&data.fen)
        .bind(if data.is_default { 1_i64 } else { 0_i64 })
        .bind(data.created_at as i64)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_finished_games(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    games: &[FinishedGameData],
) -> Result<(), PersistenceError> {
    for data in games {
        let game_mode = normalize_game_mode(&data.game_mode);

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO finished_games
                (game_id, start_fen, result, result_reason, game_mode,
                 human_side, skill_level, move_count, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&data.game_id)
        .bind(&data.start_fen)
        .bind(&data.result)
        .bind(&data.result_reason)
        .bind(game_mode)
        .bind(&data.human_side)
        .bind(data.skill_level as i64)
        .bind(data.move_count as i64)
        .bind(data.created_at as i64)
        .execute(&mut **tx)
        .await?;

        sqlx::query("DELETE FROM stored_moves WHERE game_id = ?")
            .bind(&data.game_id)
            .execute(&mut **tx)
            .await?;

        for (ply, mv) in data.moves.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO stored_moves
                    (game_id, ply, mv_from, mv_to, piece, captured,
                     promotion, san, fen_after, clock_ms)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&data.game_id)
            .bind(ply as i64)
            .bind(&mv.from)
            .bind(&mv.to)
            .bind(&mv.piece)
            .bind(&mv.captured)
            .bind(&mv.promotion)
            .bind(&mv.san)
            .bind(&mv.fen_after)
            .bind(mv.clock_ms.map(|v| v as i64))
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_reviews(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    reviews: &[GameReview],
) -> Result<(), PersistenceError> {
    for review in reviews {
        let (status, status_current_ply, status_total_plies, status_error) =
            encode_status(&review.status);

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO game_reviews
                (game_id, status, status_current_ply, status_total_plies, status_error,
                 white_accuracy, black_accuracy, total_plies, analyzed_plies, analysis_depth,
                 created_at, started_at, completed_at, winner)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&review.game_id)
        .bind(status)
        .bind(status_current_ply.map(|v| v as i64))
        .bind(status_total_plies.map(|v| v as i64))
        .bind(status_error)
        .bind(review.white_accuracy)
        .bind(review.black_accuracy)
        .bind(review.total_plies as i64)
        .bind(review.analyzed_plies as i64)
        .bind(review.analysis_depth as i64)
        .bind(now_timestamp() as i64)
        .bind(review.started_at.map(|v| v as i64))
        .bind(review.completed_at.map(|v| v as i64))
        .bind(&review.winner)
        .execute(&mut **tx)
        .await?;

        for position in &review.positions {
            let (eb_type, eb_value) = encode_score(&position.eval_before);
            let (ea_type, ea_value) = encode_score(&position.eval_after);
            let (ebest_type, ebest_value) = encode_score(&position.eval_best);
            let classification = encode_classification(&position.classification);
            let pv_json = serde_json::to_string(&position.pv)?;

            sqlx::query(
                r#"
                INSERT OR IGNORE INTO position_reviews
                    (game_id, ply, fen, played_san, best_move_san, best_move_uci,
                     eval_before_type, eval_before_value,
                     eval_after_type, eval_after_value,
                     eval_best_type, eval_best_value,
                     classification, cp_loss, pv, depth, clock_ms)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&review.game_id)
            .bind(position.ply as i64)
            .bind(&position.fen)
            .bind(&position.played_san)
            .bind(&position.best_move_san)
            .bind(&position.best_move_uci)
            .bind(eb_type)
            .bind(eb_value)
            .bind(ea_type)
            .bind(ea_value)
            .bind(ebest_type)
            .bind(ebest_value)
            .bind(classification)
            .bind(position.cp_loss as i64)
            .bind(&pv_json)
            .bind(position.depth as i64)
            .bind(position.clock_ms.map(|v| v as i64))
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_advanced_analyses(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    analyses: &[AdvancedGameAnalysis],
) -> Result<(), PersistenceError> {
    for analysis in analyses {
        sqlx::query(
            "INSERT OR REPLACE INTO advanced_game_analyses \
             (game_id, pipeline_version, shallow_depth, deep_depth, \
              critical_positions_count, computed_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&analysis.game_id)
        .bind(analysis.pipeline_version as i64)
        .bind(analysis.shallow_depth as i64)
        .bind(analysis.deep_depth as i64)
        .bind(analysis.critical_positions_count as i64)
        .bind(analysis.computed_at as i64)
        .execute(&mut **tx)
        .await?;

        sqlx::query("DELETE FROM psychological_profiles WHERE game_id = ?")
            .bind(&analysis.game_id)
            .execute(&mut **tx)
            .await?;

        for profile in [&analysis.white_psychology, &analysis.black_psychology] {
            sqlx::query(
                "INSERT INTO psychological_profiles \
                 (game_id, color, max_consecutive_errors, error_streak_start_ply, \
                  favorable_swings, unfavorable_swings, max_momentum_streak, \
                  blunder_cluster_density, blunder_cluster_range_start, blunder_cluster_range_end, \
                  time_quality_correlation, avg_blunder_time_ms, avg_good_move_time_ms, \
                  opening_avg_cp_loss, middlegame_avg_cp_loss, endgame_avg_cp_loss) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&analysis.game_id)
            .bind(profile.color.to_string())
            .bind(profile.max_consecutive_errors as i64)
            .bind(profile.error_streak_start_ply.map(|v| v as i64))
            .bind(profile.favorable_swings as i64)
            .bind(profile.unfavorable_swings as i64)
            .bind(profile.max_momentum_streak as i64)
            .bind(profile.blunder_cluster_density as i64)
            .bind(profile.blunder_cluster_range.map(|(start, _)| start as i64))
            .bind(profile.blunder_cluster_range.map(|(_, end)| end as i64))
            .bind(profile.time_quality_correlation.map(|v| v as f64))
            .bind(profile.avg_blunder_time_ms.map(|v| v as i64))
            .bind(profile.avg_good_move_time_ms.map(|v| v as i64))
            .bind(profile.opening_avg_cp_loss)
            .bind(profile.middlegame_avg_cp_loss)
            .bind(profile.endgame_avg_cp_loss)
            .execute(&mut **tx)
            .await?;
        }

        sqlx::query("DELETE FROM advanced_position_analyses WHERE game_id = ?")
            .bind(&analysis.game_id)
            .execute(&mut **tx)
            .await?;

        for position in &analysis.positions {
            let tactics_before_patterns = serde_json::to_string(&position.tactics_before.patterns)?;
            let tactics_after_patterns = serde_json::to_string(&position.tactics_after.patterns)?;

            sqlx::query(
                "INSERT INTO advanced_position_analyses \
                 (game_id, ply, is_critical, deep_depth, \
                  tension_mutually_attacked_pairs, tension_contested_squares, \
                  tension_attacked_but_defended, tension_forcing_moves, \
                  tension_checks_available, tension_captures_available, \
                  tension_volatility_score, \
                  ks_white_pawn_shield_count, ks_white_open_files_near_king, \
                  ks_white_attacker_count, ks_white_attack_weight, \
                  ks_white_attacked_king_zone_sq, ks_white_king_zone_size, \
                  ks_white_exposure_score, \
                  ks_black_pawn_shield_count, ks_black_open_files_near_king, \
                  ks_black_attacker_count, ks_black_attack_weight, \
                  ks_black_attacked_king_zone_sq, ks_black_king_zone_size, \
                  ks_black_exposure_score, \
                  tactics_before_fork_count, tactics_before_pin_count, \
                  tactics_before_skewer_count, tactics_before_discovered_attack_count, \
                  tactics_before_hanging_piece_count, tactics_before_has_back_rank_weakness, \
                  tactics_before_patterns, \
                  tactics_after_fork_count, tactics_after_pin_count, \
                  tactics_after_skewer_count, tactics_after_discovered_attack_count, \
                  tactics_after_hanging_piece_count, tactics_after_has_back_rank_weakness, \
                  tactics_after_patterns) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&analysis.game_id)
            .bind(position.ply as i64)
            .bind(if position.is_critical { 1_i64 } else { 0_i64 })
            .bind(position.deep_depth.map(|v| v as i64))
            .bind(position.tension.mutually_attacked_pairs as i64)
            .bind(position.tension.contested_squares as i64)
            .bind(position.tension.attacked_but_defended as i64)
            .bind(position.tension.forcing_moves as i64)
            .bind(position.tension.checks_available as i64)
            .bind(position.tension.captures_available as i64)
            .bind(position.tension.volatility_score as f64)
            .bind(position.king_safety.white.pawn_shield_count as i64)
            .bind(position.king_safety.white.open_files_near_king as i64)
            .bind(position.king_safety.white.attacker_count as i64)
            .bind(position.king_safety.white.attack_weight as i64)
            .bind(position.king_safety.white.attacked_king_zone_squares as i64)
            .bind(position.king_safety.white.king_zone_size as i64)
            .bind(position.king_safety.white.exposure_score as f64)
            .bind(position.king_safety.black.pawn_shield_count as i64)
            .bind(position.king_safety.black.open_files_near_king as i64)
            .bind(position.king_safety.black.attacker_count as i64)
            .bind(position.king_safety.black.attack_weight as i64)
            .bind(position.king_safety.black.attacked_king_zone_squares as i64)
            .bind(position.king_safety.black.king_zone_size as i64)
            .bind(position.king_safety.black.exposure_score as f64)
            .bind(position.tactics_before.fork_count as i64)
            .bind(position.tactics_before.pin_count as i64)
            .bind(position.tactics_before.skewer_count as i64)
            .bind(position.tactics_before.discovered_attack_count as i64)
            .bind(position.tactics_before.hanging_piece_count as i64)
            .bind(if position.tactics_before.has_back_rank_weakness {
                1_i64
            } else {
                0_i64
            })
            .bind(&tactics_before_patterns)
            .bind(position.tactics_after.fork_count as i64)
            .bind(position.tactics_after.pin_count as i64)
            .bind(position.tactics_after.skewer_count as i64)
            .bind(position.tactics_after.discovered_attack_count as i64)
            .bind(position.tactics_after.hanging_piece_count as i64)
            .bind(if position.tactics_after.has_back_rank_weakness {
                1_i64
            } else {
                0_i64
            })
            .bind(&tactics_after_patterns)
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use analysis::{
        AdvancedGameAnalysis, AdvancedPositionAnalysis, AnalysisScore, GameReview,
        KingSafetyMetrics, MoveClassification, PositionKingSafety, PositionReview,
        PositionTensionMetrics, PsychologicalProfile, ReviewStatus, TacticalAnalysis,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::sqlite::{
        Database, SqliteAdvancedAnalysisRepository, SqliteFinishedGameRepository,
        SqlitePositionRepository, SqliteReviewRepository, SqliteSessionRepository,
    };
    use crate::persistence::traits::{
        AdvancedAnalysisRepository, FinishedGameRepository, PositionRepository, ReviewRepository,
        SessionRepository,
    };
    use crate::persistence::{FinishedGameData, SavedPositionData, StoredMoveRecord, SuspendedSessionData};

    fn sample_session(id: &str, ts: u64) -> SuspendedSessionData {
        SuspendedSessionData {
            suspended_id: id.to_string(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            side_to_move: "black".to_string(),
            move_count: 1,
            game_mode: "HumanVsEngine".to_string(),
            human_side: Some("white".to_string()),
            skill_level: 10,
            created_at: ts,
        }
    }

    fn sample_position(id: &str, ts: u64, is_default: bool) -> SavedPositionData {
        SavedPositionData {
            position_id: id.to_string(),
            name: format!("Position {id}"),
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            is_default,
            created_at: ts,
        }
    }

    fn sample_game(game_id: &str, ts: u64) -> FinishedGameData {
        FinishedGameData {
            game_id: game_id.to_string(),
            start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            result: "WhiteWins".to_string(),
            result_reason: "Checkmate".to_string(),
            game_mode: "HumanVsEngine".to_string(),
            human_side: Some("white".to_string()),
            skill_level: 12,
            move_count: 2,
            moves: vec![
                StoredMoveRecord {
                    from: "e2".to_string(),
                    to: "e4".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    promotion: None,
                    san: "e4".to_string(),
                    fen_after: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
                        .to_string(),
                    clock_ms: Some(15_000),
                },
                StoredMoveRecord {
                    from: "e7".to_string(),
                    to: "e5".to_string(),
                    piece: "P".to_string(),
                    captured: None,
                    promotion: None,
                    san: "e5".to_string(),
                    fen_after: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
                        .to_string(),
                    clock_ms: Some(14_000),
                },
            ],
            created_at: ts,
        }
    }

    fn sample_review(game_id: &str) -> GameReview {
        GameReview {
            game_id: game_id.to_string(),
            status: ReviewStatus::Complete,
            positions: vec![PositionReview {
                ply: 1,
                fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
                played_san: "e4".to_string(),
                best_move_san: "e4".to_string(),
                best_move_uci: "e2e4".to_string(),
                eval_before: AnalysisScore::Centipawns(20),
                eval_after: AnalysisScore::Centipawns(25),
                eval_best: AnalysisScore::Centipawns(25),
                classification: MoveClassification::Best,
                cp_loss: 0,
                pv: vec!["e5".to_string(), "Nf3".to_string()],
                depth: 18,
                clock_ms: Some(15_000),
            }],
            white_accuracy: Some(95.0),
            black_accuracy: Some(90.0),
            total_plies: 2,
            analyzed_plies: 2,
            analysis_depth: 18,
            started_at: Some(1_000),
            completed_at: Some(2_000),
            winner: Some("White".to_string()),
        }
    }

    fn sample_tactics() -> TacticalAnalysis {
        TacticalAnalysis {
            patterns: vec![],
            fork_count: 1,
            pin_count: 0,
            skewer_count: 0,
            discovered_attack_count: 0,
            hanging_piece_count: 1,
            has_back_rank_weakness: false,
        }
    }

    fn sample_king_safety() -> PositionKingSafety {
        PositionKingSafety {
            white: KingSafetyMetrics {
                color: 'w',
                pawn_shield_count: 3,
                pawn_shield_max: 3,
                open_files_near_king: 0,
                attacker_count: 1,
                attack_weight: 4,
                attacked_king_zone_squares: 2,
                king_zone_size: 9,
                exposure_score: 0.15,
            },
            black: KingSafetyMetrics {
                color: 'b',
                pawn_shield_count: 2,
                pawn_shield_max: 3,
                open_files_near_king: 1,
                attacker_count: 2,
                attack_weight: 7,
                attacked_king_zone_squares: 4,
                king_zone_size: 9,
                exposure_score: 0.40,
            },
        }
    }

    fn sample_tension() -> PositionTensionMetrics {
        PositionTensionMetrics {
            mutually_attacked_pairs: 2,
            contested_squares: 8,
            attacked_but_defended: 3,
            forcing_moves: 4,
            checks_available: 1,
            captures_available: 3,
            volatility_score: 0.35,
        }
    }

    fn sample_profile(color: char) -> PsychologicalProfile {
        PsychologicalProfile {
            color,
            max_consecutive_errors: 2,
            error_streak_start_ply: Some(6),
            favorable_swings: 1,
            unfavorable_swings: 2,
            max_momentum_streak: 1,
            blunder_cluster_density: 1,
            blunder_cluster_range: Some((4, 8)),
            time_quality_correlation: Some(0.25),
            avg_blunder_time_ms: Some(2500),
            avg_good_move_time_ms: Some(7000),
            opening_avg_cp_loss: 14.0,
            middlegame_avg_cp_loss: 30.0,
            endgame_avg_cp_loss: 20.0,
        }
    }

    fn sample_analysis(game_id: &str, ts: u64) -> AdvancedGameAnalysis {
        AdvancedGameAnalysis {
            game_id: game_id.to_string(),
            positions: vec![AdvancedPositionAnalysis {
                ply: 1,
                tactics_before: sample_tactics(),
                tactics_after: sample_tactics(),
                king_safety: sample_king_safety(),
                tension: sample_tension(),
                is_critical: true,
                deep_depth: Some(22),
            }],
            white_psychology: sample_profile('w'),
            black_psychology: sample_profile('b'),
            pipeline_version: 1,
            shallow_depth: 10,
            deep_depth: 22,
            critical_positions_count: 1,
            computed_at: ts,
        }
    }

    fn write_json_fixtures(root: &Path) {
        let sessions = JsonStore::<SuspendedSessionData>::new(root.join("sessions"));
        let positions = JsonStore::<SavedPositionData>::new(root.join("positions"));
        let finished_games = JsonStore::<FinishedGameData>::new(root.join("finished_games"));
        let reviews = JsonStore::<GameReview>::new(root.join("reviews"));
        let advanced = JsonStore::<AdvancedGameAnalysis>::new(root.join("advanced_reviews"));

        sessions.save(&sample_session("sess_1", 1_000)).unwrap();
        sessions.save(&sample_session("sess_2", 2_000)).unwrap();
        positions.save(&sample_position("pos_1", 1_500, true)).unwrap();
        positions
            .save(&sample_position("pos_2", 1_600, false))
            .unwrap();
        finished_games.save(&sample_game("game_1", 3_000)).unwrap();
        finished_games.save(&sample_game("game_2", 4_000)).unwrap();
        reviews.save(&sample_review("game_1")).unwrap();
        reviews.save(&sample_review("game_2")).unwrap();
        advanced.save(&sample_analysis("game_1", 5_000)).unwrap();
        advanced.save(&sample_analysis("game_2", 6_000)).unwrap();
    }

    fn write_json_fixtures_doubled(root: &Path) {
        let sessions = JsonStore::<SuspendedSessionData>::new(root.join("sessions").join("sessions"));
        let positions = JsonStore::<SavedPositionData>::new(root.join("positions").join("positions"));
        let finished_games =
            JsonStore::<FinishedGameData>::new(root.join("finished_games").join("finished_games"));
        let reviews = JsonStore::<GameReview>::new(root.join("reviews").join("reviews"));
        let advanced =
            JsonStore::<AdvancedGameAnalysis>::new(root.join("advanced_reviews").join("advanced_reviews"));

        sessions.save(&sample_session("sess_1", 1_000)).unwrap();
        sessions.save(&sample_session("sess_2", 2_000)).unwrap();
        positions.save(&sample_position("pos_1", 1_500, true)).unwrap();
        positions
            .save(&sample_position("pos_2", 1_600, false))
            .unwrap();
        finished_games.save(&sample_game("game_1", 3_000)).unwrap();
        finished_games.save(&sample_game("game_2", 4_000)).unwrap();
        reviews.save(&sample_review("game_1")).unwrap();
        reviews.save(&sample_review("game_2")).unwrap();
        advanced.save(&sample_analysis("game_1", 5_000)).unwrap();
        advanced.save(&sample_analysis("game_2", 6_000)).unwrap();
    }

    async fn assert_sqlite_counts(pool: &SqlitePool) {
        let session_repo = SqliteSessionRepository::new(pool.clone());
        let position_repo = SqlitePositionRepository::new(pool.clone());
        let finished_repo = SqliteFinishedGameRepository::new(pool.clone());
        let review_repo = SqliteReviewRepository::new(pool.clone());
        let advanced_repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

        let sessions = session_repo.list_sessions().await.unwrap();
        let positions = position_repo.list_positions().await.unwrap();
        let games = finished_repo.list_games().await.unwrap();
        let reviews = review_repo.list_reviews().await.unwrap();
        let analysis_1 = advanced_repo.load_analysis("game_1").await.unwrap();
        let analysis_2 = advanced_repo.load_analysis("game_2").await.unwrap();

        assert_eq!(sessions.len(), 2);
        assert_eq!(positions.len(), 2);
        assert_eq!(games.len(), 2);
        assert_eq!(reviews.len(), 2);
        assert!(analysis_1.is_some());
        assert!(analysis_2.is_some());
    }

    #[tokio::test]
    async fn test_migration_normal_paths() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        write_json_fixtures(&data_dir);

        let db = Database::new_in_memory().await.unwrap();
        let report = migrate_json_to_sqlite(db.pool(), &data_dir).await.unwrap();

        assert!(!report.skipped);
        assert_eq!(report.sessions, 2);
        assert_eq!(report.positions, 2);
        assert_eq!(report.finished_games, 2);
        assert_eq!(report.reviews, 2);
        assert_eq!(report.advanced_analyses, 2);

        assert_sqlite_counts(db.pool()).await;
    }

    #[tokio::test]
    async fn test_migration_doubled_paths() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        write_json_fixtures_doubled(&data_dir);

        let db = Database::new_in_memory().await.unwrap();
        let report = migrate_json_to_sqlite(db.pool(), &data_dir).await.unwrap();

        assert!(!report.skipped);
        assert_eq!(report.sessions, 2);
        assert_eq!(report.positions, 2);
        assert_eq!(report.finished_games, 2);
        assert_eq!(report.reviews, 2);
        assert_eq!(report.advanced_analyses, 2);

        assert_sqlite_counts(db.pool()).await;
    }

    #[tokio::test]
    async fn test_migration_idempotency() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        write_json_fixtures(&data_dir);

        let db = Database::new_in_memory().await.unwrap();

        let first = migrate_json_to_sqlite(db.pool(), &data_dir).await.unwrap();
        assert!(!first.skipped);
        assert!(first.sessions > 0);
        assert!(first.positions > 0);
        assert!(first.finished_games > 0);
        assert!(first.reviews > 0);
        assert!(first.advanced_analyses > 0);

        let second = migrate_json_to_sqlite(db.pool(), &data_dir).await.unwrap();
        assert!(second.skipped);
        assert_eq!(second.sessions, first.sessions);
        assert_eq!(second.positions, first.positions);
        assert_eq!(second.finished_games, first.finished_games);
        assert_eq!(second.reviews, first.reviews);
        assert_eq!(second.advanced_analyses, first.advanced_analyses);
    }

    #[tokio::test]
    async fn test_migration_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let db = Database::new_in_memory().await.unwrap();

        let report = migrate_json_to_sqlite(db.pool(), &data_dir).await.unwrap();

        assert!(!report.skipped);
        assert_eq!(report.sessions, 0);
        assert_eq!(report.positions, 0);
        assert_eq!(report.finished_games, 0);
        assert_eq!(report.reviews, 0);
        assert_eq!(report.advanced_analyses, 0);
    }
}
