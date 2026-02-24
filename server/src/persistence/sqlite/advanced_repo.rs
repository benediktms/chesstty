//! SQLite-backed implementation of [`AdvancedAnalysisRepository`].

use sqlx::SqlitePool;

use crate::persistence::traits::AdvancedAnalysisRepository;
use crate::persistence::PersistenceError;
use analysis::advanced::types::{
    AdvancedGameAnalysis, AdvancedPositionAnalysis, PsychologicalProfile,
};
use analysis::board_analysis::{
    KingSafetyMetrics, PositionKingSafety, PositionTensionMetrics, TacticalTag,
};

/// SQLite implementation of [`AdvancedAnalysisRepository`].
pub struct SqliteAdvancedAnalysisRepository {
    pool: SqlitePool,
}

impl SqliteAdvancedAnalysisRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl AdvancedAnalysisRepository for SqliteAdvancedAnalysisRepository {
    async fn save_analysis(&self, analysis: &AdvancedGameAnalysis) -> Result<(), PersistenceError> {
        let pipeline_version = analysis.pipeline_version as i32;
        let shallow_depth = analysis.shallow_depth as i32;
        let deep_depth = analysis.deep_depth as i32;
        let critical_positions_count = analysis.critical_positions_count as i32;
        let computed_at = analysis.computed_at as i64;

        let mut tx = self.pool.begin().await?;

        // 1. Insert/replace the analysis header.
        sqlx::query(
            "INSERT OR REPLACE INTO advanced_game_analyses \
             (game_id, pipeline_version, shallow_depth, deep_depth, \
              critical_positions_count, computed_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&analysis.game_id)
        .bind(pipeline_version)
        .bind(shallow_depth)
        .bind(deep_depth)
        .bind(critical_positions_count)
        .bind(computed_at)
        .execute(&mut *tx)
        .await?;

        // 2. Delete existing profiles then insert white + black.
        sqlx::query("DELETE FROM psychological_profiles WHERE game_id = ?")
            .bind(&analysis.game_id)
            .execute(&mut *tx)
            .await?;

        insert_profile(&mut tx, &analysis.game_id, &analysis.white_psychology).await?;
        insert_profile(&mut tx, &analysis.game_id, &analysis.black_psychology).await?;

        // 3. Delete existing positions then batch-insert.
        sqlx::query("DELETE FROM advanced_position_analyses WHERE game_id = ?")
            .bind(&analysis.game_id)
            .execute(&mut *tx)
            .await?;

        for pos in &analysis.positions {
            insert_position(&mut tx, &analysis.game_id, pos).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn load_analysis(
        &self,
        game_id: &str,
    ) -> Result<Option<AdvancedGameAnalysis>, PersistenceError> {
        // 1. Load header.
        let header: Option<AnalysisHeaderRow> = sqlx::query_as(
            "SELECT game_id, pipeline_version, shallow_depth, deep_depth, \
                    critical_positions_count, computed_at \
             FROM advanced_game_analyses WHERE game_id = ?",
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        let header = match header {
            Some(h) => h,
            None => return Ok(None),
        };

        // 2. Load psychological profiles.
        let profile_rows: Vec<ProfileRow> = sqlx::query_as(
            "SELECT color, max_consecutive_errors, error_streak_start_ply, \
                    favorable_swings, unfavorable_swings, max_momentum_streak, \
                    blunder_cluster_density, blunder_cluster_range_start, \
                    blunder_cluster_range_end, time_quality_correlation, \
                    avg_blunder_time_ms, avg_good_move_time_ms, \
                    opening_avg_cp_loss, middlegame_avg_cp_loss, endgame_avg_cp_loss \
             FROM psychological_profiles WHERE game_id = ? ORDER BY color",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        let mut white_psychology: Option<PsychologicalProfile> = None;
        let mut black_psychology: Option<PsychologicalProfile> = None;

        for row in profile_rows {
            let profile = row.into_domain();
            match profile.color {
                'b' => black_psychology = Some(profile),
                _ => white_psychology = Some(profile),
            }
        }

        let white_psychology = white_psychology.unwrap_or_else(|| default_profile('w'));
        let black_psychology = black_psychology.unwrap_or_else(|| default_profile('b'));

        // 3. Load position analyses.
        let pos_rows: Vec<PositionRow> = sqlx::query_as(
            "SELECT ply, is_critical, deep_depth, \
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
                    tactics_before_tags, tactics_after_tags \
             FROM advanced_position_analyses WHERE game_id = ? ORDER BY ply",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        let positions: Result<Vec<AdvancedPositionAnalysis>, PersistenceError> =
            pos_rows.into_iter().map(|r| r.into_domain()).collect();

        Ok(Some(AdvancedGameAnalysis {
            game_id: header.game_id,
            positions: positions?,
            white_psychology,
            black_psychology,
            pipeline_version: header.pipeline_version as u32,
            shallow_depth: header.shallow_depth as u32,
            deep_depth: header.deep_depth as u32,
            critical_positions_count: header.critical_positions_count as u32,
            computed_at: header.computed_at as u64,
        }))
    }

    async fn delete_analysis(&self, game_id: &str) -> Result<(), PersistenceError> {
        // CASCADE handles child tables.
        sqlx::query("DELETE FROM advanced_game_analyses WHERE game_id = ?")
            .bind(game_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn insert_profile(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    game_id: &str,
    profile: &PsychologicalProfile,
) -> Result<(), PersistenceError> {
    let color = profile.color.to_string();
    let max_consecutive_errors = profile.max_consecutive_errors as i32;
    let error_streak_start_ply = profile.error_streak_start_ply.map(|v| v as i32);
    let favorable_swings = profile.favorable_swings as i32;
    let unfavorable_swings = profile.unfavorable_swings as i32;
    let max_momentum_streak = profile.max_momentum_streak as i32;
    let blunder_cluster_density = profile.blunder_cluster_density as i32;
    let blunder_cluster_range_start = profile.blunder_cluster_range.map(|(s, _)| s as i32);
    let blunder_cluster_range_end = profile.blunder_cluster_range.map(|(_, e)| e as i32);
    let time_quality_correlation = profile.time_quality_correlation.map(|v| v as f64);
    let avg_blunder_time_ms = profile.avg_blunder_time_ms.map(|v| v as i64);
    let avg_good_move_time_ms = profile.avg_good_move_time_ms.map(|v| v as i64);

    sqlx::query(
        "INSERT INTO psychological_profiles \
         (game_id, color, max_consecutive_errors, error_streak_start_ply, \
          favorable_swings, unfavorable_swings, max_momentum_streak, \
          blunder_cluster_density, blunder_cluster_range_start, blunder_cluster_range_end, \
          time_quality_correlation, avg_blunder_time_ms, avg_good_move_time_ms, \
          opening_avg_cp_loss, middlegame_avg_cp_loss, endgame_avg_cp_loss) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(game_id)
    .bind(&color)
    .bind(max_consecutive_errors)
    .bind(error_streak_start_ply)
    .bind(favorable_swings)
    .bind(unfavorable_swings)
    .bind(max_momentum_streak)
    .bind(blunder_cluster_density)
    .bind(blunder_cluster_range_start)
    .bind(blunder_cluster_range_end)
    .bind(time_quality_correlation)
    .bind(avg_blunder_time_ms)
    .bind(avg_good_move_time_ms)
    .bind(profile.opening_avg_cp_loss)
    .bind(profile.middlegame_avg_cp_loss)
    .bind(profile.endgame_avg_cp_loss)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_position(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    game_id: &str,
    pos: &AdvancedPositionAnalysis,
) -> Result<(), PersistenceError> {
    let ply = pos.ply as i32;
    let is_critical: i32 = if pos.is_critical { 1 } else { 0 };
    let deep_depth = pos.deep_depth.map(|v| v as i32);

    // Tension
    let t_mutually = pos.tension.mutually_attacked_pairs as i32;
    let t_contested = pos.tension.contested_squares as i32;
    let t_defended = pos.tension.attacked_but_defended as i32;
    let t_forcing = pos.tension.forcing_moves as i32;
    let t_checks = pos.tension.checks_available as i32;
    let t_captures = pos.tension.captures_available as i32;
    let t_volatility = pos.tension.volatility_score as f64;

    // King safety white
    let ksw_pawn = pos.king_safety.white.pawn_shield_count as i32;
    let ksw_open = pos.king_safety.white.open_files_near_king as i32;
    let ksw_attacker = pos.king_safety.white.attacker_count as i32;
    let ksw_weight = pos.king_safety.white.attack_weight as i32;
    let ksw_zone_sq = pos.king_safety.white.attacked_king_zone_squares as i32;
    let ksw_zone_size = pos.king_safety.white.king_zone_size as i32;
    let ksw_exposure = pos.king_safety.white.exposure_score as f64;

    // King safety black
    let ksb_pawn = pos.king_safety.black.pawn_shield_count as i32;
    let ksb_open = pos.king_safety.black.open_files_near_king as i32;
    let ksb_attacker = pos.king_safety.black.attacker_count as i32;
    let ksb_weight = pos.king_safety.black.attack_weight as i32;
    let ksb_zone_sq = pos.king_safety.black.attacked_king_zone_squares as i32;
    let ksb_zone_size = pos.king_safety.black.king_zone_size as i32;
    let ksb_exposure = pos.king_safety.black.exposure_score as f64;

    let tb_tags =
        serde_json::to_string(&pos.tactical_tags_before).unwrap_or_else(|_| "[]".to_string());
    let ta_tags =
        serde_json::to_string(&pos.tactical_tags_after).unwrap_or_else(|_| "[]".to_string());

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
          tactics_before_tags, tactics_after_tags) \
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(game_id)
    .bind(ply)
    .bind(is_critical)
    .bind(deep_depth)
    .bind(t_mutually)
    .bind(t_contested)
    .bind(t_defended)
    .bind(t_forcing)
    .bind(t_checks)
    .bind(t_captures)
    .bind(t_volatility)
    .bind(ksw_pawn)
    .bind(ksw_open)
    .bind(ksw_attacker)
    .bind(ksw_weight)
    .bind(ksw_zone_sq)
    .bind(ksw_zone_size)
    .bind(ksw_exposure)
    .bind(ksb_pawn)
    .bind(ksb_open)
    .bind(ksb_attacker)
    .bind(ksb_weight)
    .bind(ksb_zone_sq)
    .bind(ksb_zone_size)
    .bind(ksb_exposure)
    .bind(tb_tags)
    .bind(ta_tags)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn default_profile(color: char) -> PsychologicalProfile {
    PsychologicalProfile {
        color,
        max_consecutive_errors: 0,
        error_streak_start_ply: None,
        favorable_swings: 0,
        unfavorable_swings: 0,
        max_momentum_streak: 0,
        blunder_cluster_density: 0,
        blunder_cluster_range: None,
        time_quality_correlation: None,
        avg_blunder_time_ms: None,
        avg_good_move_time_ms: None,
        opening_avg_cp_loss: 0.0,
        middlegame_avg_cp_loss: 0.0,
        endgame_avg_cp_loss: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Row types for `query_as` tuple mapping
// ---------------------------------------------------------------------------

/// Row type for analysis header queries, mapped via `sqlx::FromRow`.
#[derive(sqlx::FromRow)]
struct AnalysisHeaderRow {
    game_id: String,
    pipeline_version: i64,
    shallow_depth: i64,
    deep_depth: i64,
    critical_positions_count: i64,
    computed_at: i64,
}

/// Maps a row from `psychological_profiles`.
#[derive(sqlx::FromRow)]
struct ProfileRow {
    color: String,
    max_consecutive_errors: i64,
    error_streak_start_ply: Option<i64>,
    favorable_swings: i64,
    unfavorable_swings: i64,
    max_momentum_streak: i64,
    blunder_cluster_density: i64,
    blunder_cluster_range_start: Option<i64>,
    blunder_cluster_range_end: Option<i64>,
    time_quality_correlation: Option<f64>,
    avg_blunder_time_ms: Option<i64>,
    avg_good_move_time_ms: Option<i64>,
    opening_avg_cp_loss: f64,
    middlegame_avg_cp_loss: f64,
    endgame_avg_cp_loss: f64,
}

impl ProfileRow {
    fn into_domain(self) -> PsychologicalProfile {
        let blunder_cluster_range = match (
            self.blunder_cluster_range_start,
            self.blunder_cluster_range_end,
        ) {
            (Some(s), Some(e)) => Some((s as u32, e as u32)),
            _ => None,
        };

        PsychologicalProfile {
            color: self.color.chars().next().unwrap_or('w'),
            max_consecutive_errors: self.max_consecutive_errors as u8,
            error_streak_start_ply: self.error_streak_start_ply.map(|v| v as u32),
            favorable_swings: self.favorable_swings as u8,
            unfavorable_swings: self.unfavorable_swings as u8,
            max_momentum_streak: self.max_momentum_streak as u8,
            blunder_cluster_density: self.blunder_cluster_density as u8,
            blunder_cluster_range,
            time_quality_correlation: self.time_quality_correlation.map(|v| v as f32),
            avg_blunder_time_ms: self.avg_blunder_time_ms.map(|v| v as u64),
            avg_good_move_time_ms: self.avg_good_move_time_ms.map(|v| v as u64),
            opening_avg_cp_loss: self.opening_avg_cp_loss,
            middlegame_avg_cp_loss: self.middlegame_avg_cp_loss,
            endgame_avg_cp_loss: self.endgame_avg_cp_loss,
        }
    }
}

/// Maps a row from `advanced_position_analyses`.
#[derive(sqlx::FromRow)]
struct PositionRow {
    ply: i64,
    is_critical: i64,
    deep_depth: Option<i64>,
    // Tension
    tension_mutually_attacked_pairs: i64,
    tension_contested_squares: i64,
    tension_attacked_but_defended: i64,
    tension_forcing_moves: i64,
    tension_checks_available: i64,
    tension_captures_available: i64,
    tension_volatility_score: f64,
    // King safety white
    ks_white_pawn_shield_count: i64,
    ks_white_open_files_near_king: i64,
    ks_white_attacker_count: i64,
    ks_white_attack_weight: i64,
    ks_white_attacked_king_zone_sq: i64,
    ks_white_king_zone_size: i64,
    ks_white_exposure_score: f64,
    // King safety black
    ks_black_pawn_shield_count: i64,
    ks_black_open_files_near_king: i64,
    ks_black_attacker_count: i64,
    ks_black_attack_weight: i64,
    ks_black_attacked_king_zone_sq: i64,
    ks_black_king_zone_size: i64,
    ks_black_exposure_score: f64,
    // Tactical tags (JSON)
    tactics_before_tags: String,
    tactics_after_tags: String,
}

impl PositionRow {
    fn into_domain(self) -> Result<AdvancedPositionAnalysis, PersistenceError> {
        let tactical_tags_before: Vec<TacticalTag> =
            serde_json::from_str(&self.tactics_before_tags).unwrap_or_default();
        let tactical_tags_after: Vec<TacticalTag> =
            serde_json::from_str(&self.tactics_after_tags).unwrap_or_default();

        Ok(AdvancedPositionAnalysis {
            ply: self.ply as u32,
            is_critical: self.is_critical != 0,
            deep_depth: self.deep_depth.map(|v| v as u32),
            tension: PositionTensionMetrics {
                mutually_attacked_pairs: self.tension_mutually_attacked_pairs as u8,
                contested_squares: self.tension_contested_squares as u8,
                attacked_but_defended: self.tension_attacked_but_defended as u8,
                forcing_moves: self.tension_forcing_moves as u8,
                checks_available: self.tension_checks_available as u8,
                captures_available: self.tension_captures_available as u8,
                volatility_score: self.tension_volatility_score as f32,
            },
            king_safety: PositionKingSafety {
                white: KingSafetyMetrics {
                    color: 'w',
                    pawn_shield_count: self.ks_white_pawn_shield_count as u8,
                    pawn_shield_max: 3,
                    open_files_near_king: self.ks_white_open_files_near_king as u8,
                    attacker_count: self.ks_white_attacker_count as u8,
                    attack_weight: self.ks_white_attack_weight as u16,
                    attacked_king_zone_squares: self.ks_white_attacked_king_zone_sq as u8,
                    king_zone_size: self.ks_white_king_zone_size as u8,
                    exposure_score: self.ks_white_exposure_score as f32,
                },
                black: KingSafetyMetrics {
                    color: 'b',
                    pawn_shield_count: self.ks_black_pawn_shield_count as u8,
                    pawn_shield_max: 3,
                    open_files_near_king: self.ks_black_open_files_near_king as u8,
                    attacker_count: self.ks_black_attacker_count as u8,
                    attack_weight: self.ks_black_attack_weight as u16,
                    attacked_king_zone_squares: self.ks_black_attacked_king_zone_sq as u8,
                    king_zone_size: self.ks_black_king_zone_size as u8,
                    exposure_score: self.ks_black_exposure_score as f32,
                },
            },
            tactical_tags_before,
            tactical_tags_after,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::Database;

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
                attacker_count: 3,
                attack_weight: 8,
                attacked_king_zone_squares: 5,
                king_zone_size: 9,
                exposure_score: 0.55,
            },
        }
    }

    fn sample_tension() -> PositionTensionMetrics {
        PositionTensionMetrics {
            mutually_attacked_pairs: 3,
            contested_squares: 12,
            attacked_but_defended: 4,
            forcing_moves: 6,
            checks_available: 1,
            captures_available: 5,
            volatility_score: 0.42,
        }
    }

    fn sample_profile(color: char) -> PsychologicalProfile {
        PsychologicalProfile {
            color,
            max_consecutive_errors: 3,
            error_streak_start_ply: Some(14),
            favorable_swings: 2,
            unfavorable_swings: 4,
            max_momentum_streak: 2,
            blunder_cluster_density: 2,
            blunder_cluster_range: Some((10, 20)),
            time_quality_correlation: Some(0.35),
            avg_blunder_time_ms: Some(2500),
            avg_good_move_time_ms: Some(8000),
            opening_avg_cp_loss: 15.5,
            middlegame_avg_cp_loss: 42.3,
            endgame_avg_cp_loss: 28.1,
        }
    }

    fn sample_analysis() -> AdvancedGameAnalysis {
        AdvancedGameAnalysis {
            game_id: "test_game_1".to_string(),
            positions: vec![
                AdvancedPositionAnalysis {
                    ply: 0,
                    tactical_tags_before: vec![],
                    tactical_tags_after: vec![],
                    king_safety: sample_king_safety(),
                    tension: sample_tension(),
                    is_critical: false,
                    deep_depth: None,
                },
                AdvancedPositionAnalysis {
                    ply: 1,
                    tactical_tags_before: vec![],
                    tactical_tags_after: vec![],
                    king_safety: sample_king_safety(),
                    tension: sample_tension(),
                    is_critical: true,
                    deep_depth: Some(22),
                },
            ],
            white_psychology: sample_profile('w'),
            black_psychology: sample_profile('b'),
            pipeline_version: 1,
            shallow_depth: 10,
            deep_depth: 22,
            critical_positions_count: 1,
            computed_at: 1700000000,
        }
    }

    /// Helper: save a finished game stub so that the FK from
    /// `advanced_game_analyses` to `finished_games` is satisfied.
    async fn insert_finished_game_stub(pool: &SqlitePool, game_id: &str) {
        sqlx::query(
            "INSERT INTO finished_games \
             (game_id, start_fen, result, result_reason, game_mode, \
              human_side, skill_level, move_count, created_at) \
             VALUES (?, '', 'Draw', 'Agreement', 'HumanVsEngine', 'white', 10, 0, 0)",
        )
        .bind(game_id)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let db = Database::new_in_memory().await.unwrap();
        let pool = db.pool().clone();
        let repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

        insert_finished_game_stub(&pool, "test_game_1").await;

        let original = sample_analysis();
        repo.save_analysis(&original).await.unwrap();

        let loaded = repo.load_analysis("test_game_1").await.unwrap().unwrap();

        // Header fields
        assert_eq!(loaded.game_id, original.game_id);
        assert_eq!(loaded.pipeline_version, original.pipeline_version);
        assert_eq!(loaded.shallow_depth, original.shallow_depth);
        assert_eq!(loaded.deep_depth, original.deep_depth);
        assert_eq!(
            loaded.critical_positions_count,
            original.critical_positions_count
        );
        assert_eq!(loaded.computed_at, original.computed_at);

        // Positions
        assert_eq!(loaded.positions.len(), 2);
        assert_eq!(loaded.positions[0].ply, 0);
        assert!(!loaded.positions[0].is_critical);
        assert_eq!(loaded.positions[0].deep_depth, None);
        assert_eq!(loaded.positions[1].ply, 1);
        assert!(loaded.positions[1].is_critical);
        assert_eq!(loaded.positions[1].deep_depth, Some(22));

        // Tension
        assert_eq!(loaded.positions[0].tension.mutually_attacked_pairs, 3);
        assert_eq!(loaded.positions[0].tension.contested_squares, 12);
        let vol = loaded.positions[0].tension.volatility_score;
        assert!(
            (vol - 0.42).abs() < 0.001,
            "volatility_score mismatch: {vol}"
        );

        // King safety
        assert_eq!(loaded.positions[0].king_safety.white.pawn_shield_count, 3);
        assert_eq!(loaded.positions[0].king_safety.white.pawn_shield_max, 3);
        assert_eq!(loaded.positions[0].king_safety.black.attacker_count, 3);
        let exp = loaded.positions[0].king_safety.black.exposure_score;
        assert!((exp - 0.55).abs() < 0.01, "exposure_score mismatch: {exp}");

        // Tactical tags (empty in sample data)
        assert!(loaded.positions[0].tactical_tags_before.is_empty());
        assert!(loaded.positions[0].tactical_tags_after.is_empty());

        // Psychology - white
        let wp = &loaded.white_psychology;
        assert_eq!(wp.color, 'w');
        assert_eq!(wp.max_consecutive_errors, 3);
        assert_eq!(wp.error_streak_start_ply, Some(14));
        assert_eq!(wp.favorable_swings, 2);
        assert_eq!(wp.unfavorable_swings, 4);
        assert_eq!(wp.blunder_cluster_range, Some((10, 20)));
        let tqc = wp.time_quality_correlation.unwrap();
        assert!(
            (tqc - 0.35).abs() < 0.001,
            "time_quality_correlation mismatch: {tqc}"
        );
        assert_eq!(wp.avg_blunder_time_ms, Some(2500));
        assert!((wp.opening_avg_cp_loss - 15.5).abs() < 0.001);

        // Psychology - black
        let bp = &loaded.black_psychology;
        assert_eq!(bp.color, 'b');
        assert_eq!(bp.max_consecutive_errors, 3);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let db = Database::new_in_memory().await.unwrap();
        let repo = SqliteAdvancedAnalysisRepository::new(db.pool().clone());

        let loaded = repo.load_analysis("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_delete_cascades() {
        let db = Database::new_in_memory().await.unwrap();
        let pool = db.pool().clone();
        let repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

        insert_finished_game_stub(&pool, "test_game_1").await;

        repo.save_analysis(&sample_analysis()).await.unwrap();
        repo.delete_analysis("test_game_1").await.unwrap();

        let loaded = repo.load_analysis("test_game_1").await.unwrap();
        assert!(loaded.is_none());

        // Verify child tables are empty via cascade.
        let profile_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM psychological_profiles WHERE game_id = 'test_game_1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(profile_count.0, 0);

        let pos_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM advanced_position_analyses WHERE game_id = 'test_game_1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pos_count.0, 0);
    }

    #[tokio::test]
    async fn test_save_replace_updates() {
        let db = Database::new_in_memory().await.unwrap();
        let pool = db.pool().clone();
        let repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

        insert_finished_game_stub(&pool, "test_game_1").await;

        let mut analysis = sample_analysis();
        repo.save_analysis(&analysis).await.unwrap();

        // Update and re-save.
        analysis.pipeline_version = 2;
        analysis.positions = vec![analysis.positions[0].clone()];
        analysis.white_psychology.max_consecutive_errors = 10;
        repo.save_analysis(&analysis).await.unwrap();

        let loaded = repo.load_analysis("test_game_1").await.unwrap().unwrap();
        assert_eq!(loaded.pipeline_version, 2);
        assert_eq!(loaded.positions.len(), 1);
        assert_eq!(loaded.white_psychology.max_consecutive_errors, 10);
    }
}
