use analysis::{
    AdvancedGameAnalysis, AdvancedPositionAnalysis, AnalysisScore, GameReview, KingSafetyMetrics,
    MoveClassification, PositionKingSafety, PositionReview, PositionTensionMetrics,
    PsychologicalProfile, ReviewStatus,
};

use super::{
    Database, SqliteAdvancedAnalysisRepository, SqliteFinishedGameRepository,
    SqlitePositionRepository, SqliteReviewRepository, SqliteSessionRepository,
};
use crate::persistence::traits::{
    AdvancedAnalysisRepository, FinishedGameRepository, PositionRepository, ReviewRepository,
    SessionRepository,
};
use crate::persistence::{
    FinishedGameData, PersistenceError, SavedPositionData, StoredMoveRecord, SuspendedSessionData,
};

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

fn sample_position(id: &str, is_default: bool, ts: u64) -> SavedPositionData {
    SavedPositionData {
        position_id: id.to_string(),
        name: format!("Position {id}"),
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        is_default,
        created_at: ts,
    }
}

fn sample_moves() -> Vec<StoredMoveRecord> {
    vec![
        StoredMoveRecord {
            from: "e2".to_string(),
            to: "e4".to_string(),
            piece: "P".to_string(),
            captured: None,
            promotion: None,
            san: "e4".to_string(),
            fen_after: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
            clock_ms: Some(15_000),
        },
        StoredMoveRecord {
            from: "c7".to_string(),
            to: "c5".to_string(),
            piece: "P".to_string(),
            captured: None,
            promotion: None,
            san: "c5".to_string(),
            fen_after: "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2".to_string(),
            clock_ms: Some(14_000),
        },
        StoredMoveRecord {
            from: "g1".to_string(),
            to: "f3".to_string(),
            piece: "N".to_string(),
            captured: None,
            promotion: None,
            san: "Nf3".to_string(),
            fen_after: "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".to_string(),
            clock_ms: Some(13_500),
        },
        StoredMoveRecord {
            from: "d7".to_string(),
            to: "d6".to_string(),
            piece: "P".to_string(),
            captured: None,
            promotion: None,
            san: "d6".to_string(),
            fen_after: "rnbqkbnr/pp2pppp/3p4/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3".to_string(),
            clock_ms: Some(13_000),
        },
    ]
}

fn sample_finished_game(id: &str, ts: u64) -> FinishedGameData {
    let moves = sample_moves();
    FinishedGameData {
        game_id: id.to_string(),
        start_fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
        result: "WhiteWins".to_string(),
        result_reason: "Checkmate".to_string(),
        game_mode: "HumanVsEngine".to_string(),
        human_side: Some("white".to_string()),
        skill_level: 12,
        move_count: moves.len() as u32,
        moves,
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
            pv: vec!["c5".to_string(), "Nf3".to_string()],
            depth: 18,
            clock_ms: Some(15_000),
        }],
        white_accuracy: Some(96.0),
        black_accuracy: Some(92.0),
        total_plies: 4,
        analyzed_plies: 4,
        analysis_depth: 18,
        started_at: Some(1_000),
        completed_at: Some(2_000),
        winner: Some("White".to_string()),
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
            exposure_score: 0.42,
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
        volatility_score: 0.37,
    }
}

fn sample_profile(color: char) -> PsychologicalProfile {
    PsychologicalProfile {
        color,
        max_consecutive_errors: 2,
        error_streak_start_ply: Some(5),
        favorable_swings: 1,
        unfavorable_swings: 2,
        max_momentum_streak: 1,
        blunder_cluster_density: 1,
        blunder_cluster_range: Some((3, 7)),
        time_quality_correlation: Some(0.30),
        avg_blunder_time_ms: Some(2400),
        avg_good_move_time_ms: Some(7100),
        opening_avg_cp_loss: 12.0,
        middlegame_avg_cp_loss: 22.0,
        endgame_avg_cp_loss: 18.0,
    }
}

fn sample_analysis(game_id: &str, ts: u64) -> AdvancedGameAnalysis {
    AdvancedGameAnalysis {
        game_id: game_id.to_string(),
        positions: vec![AdvancedPositionAnalysis {
            ply: 1,
            tactical_tags_before: vec![],
            tactical_tags_after: vec![],
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

#[tokio::test]
async fn test_session_lifecycle_sqlite() {
    let db = Database::new_in_memory().await.unwrap();
    let repo = SqliteSessionRepository::new(db.pool().clone());
    let data = sample_session("sess_lifecycle", 1_000);

    repo.save_session(&data).await.unwrap();

    let listed = repo.list_sessions().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].suspended_id, "sess_lifecycle");

    let loaded = repo.load_session("sess_lifecycle").await.unwrap();
    assert_eq!(loaded, Some(data));

    repo.delete_session("sess_lifecycle").await.unwrap();
    let after = repo.load_session("sess_lifecycle").await.unwrap();
    assert!(after.is_none());
}

#[tokio::test]
async fn test_finished_game_with_moves_integrity() {
    let db = Database::new_in_memory().await.unwrap();
    let repo = SqliteFinishedGameRepository::new(db.pool().clone());
    let game = sample_finished_game("game_integrity", 2_000);

    repo.save_game(&game).await.unwrap();

    let loaded = repo.load_game("game_integrity").await.unwrap().unwrap();
    assert_eq!(loaded.moves.len(), 4);
    assert_eq!(loaded.moves[0].san, "e4");
    assert_eq!(loaded.moves[1].san, "c5");
    assert_eq!(loaded.moves[2].san, "Nf3");
    assert_eq!(loaded.moves[3].san, "d6");
    assert_eq!(loaded.moves[0].fen_after, "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
    assert_eq!(loaded.moves[3].fen_after, "rnbqkbnr/pp2pppp/3p4/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3");
    assert_eq!(loaded.moves[0].clock_ms, Some(15_000));
    assert_eq!(loaded.moves[3].clock_ms, Some(13_000));
}

#[tokio::test]
async fn test_review_and_analysis_cross_reference() {
    let db = Database::new_in_memory().await.unwrap();
    let pool = db.pool().clone();
    let finished_repo = SqliteFinishedGameRepository::new(pool.clone());
    let review_repo = SqliteReviewRepository::new(pool.clone());
    let analysis_repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

    let game = sample_finished_game("game_cross_ref", 3_000);
    let review = sample_review("game_cross_ref");
    let analysis = sample_analysis("game_cross_ref", 4_000);

    finished_repo.save_game(&game).await.unwrap();
    review_repo.save_review(&review).await.unwrap();
    analysis_repo.save_analysis(&analysis).await.unwrap();

    let loaded_game = finished_repo.load_game("game_cross_ref").await.unwrap();
    let loaded_review = review_repo.load_review("game_cross_ref").await.unwrap();
    let loaded_analysis = analysis_repo.load_analysis("game_cross_ref").await.unwrap();

    assert!(loaded_game.is_some());
    assert!(loaded_review.is_some());
    assert!(loaded_analysis.is_some());
    assert_eq!(loaded_review.unwrap().game_id, "game_cross_ref");
    assert_eq!(loaded_analysis.unwrap().game_id, "game_cross_ref");
}

#[tokio::test]
async fn test_position_default_protection() {
    let db = Database::new_in_memory().await.unwrap();
    let repo = SqlitePositionRepository::new(db.pool().clone());

    repo.save_position(&sample_position("pos_default", true, 100))
        .await
        .unwrap();

    let protected_err = repo.delete_position("pos_default").await;
    assert!(matches!(
        protected_err,
        Err(PersistenceError::DefaultPositionProtected)
    ));

    repo.save_position(&sample_position("pos_user", false, 101))
        .await
        .unwrap();
    repo.delete_position("pos_user").await.unwrap();

    let listed = repo.list_positions().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].position_id, "pos_default");
}

#[tokio::test]
async fn test_concurrent_repo_access() {
    let db = Database::new_in_memory().await.unwrap();
    let pool = db.pool().clone();

    let sessions_pool = pool.clone();
    let positions_pool = pool.clone();
    let review_flow_pool = pool.clone();

    let sessions_task = tokio::spawn(async move {
        let repo = SqliteSessionRepository::new(sessions_pool);
        for i in 0..10_u64 {
            let id = format!("sess_concurrent_{i}");
            repo.save_session(&sample_session(&id, 1_000 + i)).await.unwrap();
        }
    });

    let positions_task = tokio::spawn(async move {
        let repo = SqlitePositionRepository::new(positions_pool);
        for i in 0..10_u64 {
            let id = format!("pos_concurrent_{i}");
            repo.save_position(&sample_position(&id, false, 2_000 + i))
                .await
                .unwrap();
        }
    });

    let review_flow_task = tokio::spawn(async move {
        let finished_repo = SqliteFinishedGameRepository::new(review_flow_pool.clone());
        let review_repo = SqliteReviewRepository::new(review_flow_pool.clone());
        let analysis_repo = SqliteAdvancedAnalysisRepository::new(review_flow_pool);

        for i in 0..6_u64 {
            let game_id = format!("game_concurrent_{i}");
            finished_repo
                .save_game(&sample_finished_game(&game_id, 3_000 + i))
                .await
                .unwrap();
            review_repo.save_review(&sample_review(&game_id)).await.unwrap();
            analysis_repo
                .save_analysis(&sample_analysis(&game_id, 4_000 + i))
                .await
                .unwrap();
        }
    });

    sessions_task.await.unwrap();
    positions_task.await.unwrap();
    review_flow_task.await.unwrap();

    let session_repo = SqliteSessionRepository::new(pool.clone());
    let position_repo = SqlitePositionRepository::new(pool.clone());
    let finished_repo = SqliteFinishedGameRepository::new(pool.clone());
    let review_repo = SqliteReviewRepository::new(pool.clone());
    let analysis_repo = SqliteAdvancedAnalysisRepository::new(pool.clone());

    assert_eq!(session_repo.list_sessions().await.unwrap().len(), 10);
    assert_eq!(position_repo.list_positions().await.unwrap().len(), 10);
    assert_eq!(finished_repo.list_games().await.unwrap().len(), 6);
    assert_eq!(review_repo.list_reviews().await.unwrap().len(), 6);
    assert!(analysis_repo
        .load_analysis("game_concurrent_0")
        .await
        .unwrap()
        .is_some());
    assert!(analysis_repo
        .load_analysis("game_concurrent_5")
        .await
        .unwrap()
        .is_some());
}
