-- ChessTTY SQLite Schema v1
-- All tables use STRICT mode for type safety

CREATE TABLE suspended_sessions (
    suspended_id TEXT PRIMARY KEY NOT NULL,
    fen          TEXT NOT NULL,
    side_to_move TEXT NOT NULL CHECK(side_to_move IN ('white', 'black')),
    move_count   INTEGER NOT NULL,
    game_mode    TEXT NOT NULL CHECK(game_mode IN ('HumanVsEngine', 'HumanVsHuman', 'EngineVsEngine', 'Analysis', 'Review')),
    human_side   TEXT CHECK(human_side IN ('white', 'black')),
    skill_level  INTEGER NOT NULL CHECK(skill_level BETWEEN 0 AND 20),
    created_at   INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_sessions_created_at ON suspended_sessions(created_at DESC);

CREATE TABLE saved_positions (
    position_id TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    fen         TEXT NOT NULL,
    is_default  INTEGER NOT NULL DEFAULT 0 CHECK(is_default IN (0, 1)),
    created_at  INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_positions_list_order ON saved_positions(is_default DESC, created_at DESC);

CREATE TABLE finished_games (
    game_id       TEXT PRIMARY KEY NOT NULL,
    start_fen     TEXT NOT NULL,
    result        TEXT NOT NULL CHECK(result IN ('WhiteWins', 'BlackWins', 'Draw')),
    result_reason TEXT NOT NULL,
    game_mode     TEXT NOT NULL CHECK(game_mode IN ('HumanVsEngine', 'HumanVsHuman', 'EngineVsEngine', 'Analysis', 'Review')),
    human_side    TEXT CHECK(human_side IN ('white', 'black')),
    skill_level   INTEGER NOT NULL CHECK(skill_level BETWEEN 0 AND 20),
    move_count    INTEGER NOT NULL,
    created_at    INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_finished_games_created_at ON finished_games(created_at DESC);

CREATE TABLE stored_moves (
    id        INTEGER PRIMARY KEY,
    game_id   TEXT NOT NULL REFERENCES finished_games(game_id) ON DELETE CASCADE,
    ply       INTEGER NOT NULL,
    mv_from   TEXT NOT NULL,
    mv_to     TEXT NOT NULL,
    piece     TEXT NOT NULL CHECK(piece IN ('P','N','B','R','Q','K')),
    captured  TEXT CHECK(captured IN ('P','N','B','R','Q','K')),
    promotion TEXT CHECK(promotion IN ('N','B','R','Q')),
    san       TEXT NOT NULL,
    fen_after TEXT NOT NULL,
    clock_ms  INTEGER,
    UNIQUE(game_id, ply)
) STRICT;

CREATE TABLE game_reviews (
    game_id            TEXT PRIMARY KEY NOT NULL
                       REFERENCES finished_games(game_id) ON DELETE CASCADE,
    status             TEXT NOT NULL CHECK(status IN ('Queued','Analyzing','Complete','Failed')),
    status_current_ply INTEGER,
    status_total_plies INTEGER,
    status_error       TEXT,
    white_accuracy     REAL,
    black_accuracy     REAL,
    total_plies        INTEGER NOT NULL,
    analyzed_plies     INTEGER NOT NULL DEFAULT 0 CHECK(analyzed_plies >= 0),
    analysis_depth     INTEGER NOT NULL,
    created_at         INTEGER NOT NULL,
    started_at         INTEGER,
    completed_at       INTEGER,
    winner             TEXT CHECK(winner IN ('White', 'Black', 'Draw')),
    CHECK(
        (status = 'Analyzing' AND status_current_ply IS NOT NULL AND status_total_plies IS NOT NULL)
        OR (status = 'Failed' AND status_error IS NOT NULL)
        OR (status IN ('Queued', 'Complete'))
    )
) STRICT;
CREATE INDEX idx_game_reviews_pending ON game_reviews(status)
    WHERE status IN ('Queued', 'Analyzing');

CREATE TABLE position_reviews (
    id               INTEGER PRIMARY KEY,
    game_id          TEXT NOT NULL REFERENCES game_reviews(game_id) ON DELETE CASCADE,
    ply              INTEGER NOT NULL,
    fen              TEXT NOT NULL,
    played_san       TEXT NOT NULL,
    best_move_san    TEXT NOT NULL,
    best_move_uci    TEXT NOT NULL,
    eval_before_type  TEXT NOT NULL CHECK(eval_before_type IN ('cp', 'mate')),
    eval_before_value INTEGER NOT NULL,
    eval_after_type   TEXT NOT NULL CHECK(eval_after_type IN ('cp', 'mate')),
    eval_after_value  INTEGER NOT NULL,
    eval_best_type    TEXT NOT NULL CHECK(eval_best_type IN ('cp', 'mate')),
    eval_best_value   INTEGER NOT NULL,
    classification   TEXT NOT NULL CHECK(classification IN (
                        'Brilliant','Best','Excellent','Good',
                        'Inaccuracy','Mistake','Blunder','Forced','Book'
                     )),
    cp_loss          INTEGER NOT NULL CHECK(cp_loss >= 0),
    pv               TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(pv)),
    depth            INTEGER NOT NULL,
    clock_ms         INTEGER,
    UNIQUE(game_id, ply)
) STRICT;

CREATE TABLE advanced_game_analyses (
    game_id                  TEXT PRIMARY KEY NOT NULL
                             REFERENCES finished_games(game_id) ON DELETE CASCADE,
    pipeline_version         INTEGER NOT NULL,
    shallow_depth            INTEGER NOT NULL,
    deep_depth               INTEGER NOT NULL,
    critical_positions_count INTEGER NOT NULL,
    computed_at              INTEGER NOT NULL
) STRICT;

CREATE TABLE psychological_profiles (
    id                          INTEGER PRIMARY KEY,
    game_id                     TEXT NOT NULL
                                REFERENCES advanced_game_analyses(game_id) ON DELETE CASCADE,
    color                       TEXT NOT NULL CHECK(color IN ('w', 'b')),
    max_consecutive_errors      INTEGER NOT NULL,
    error_streak_start_ply      INTEGER,
    favorable_swings            INTEGER NOT NULL,
    unfavorable_swings          INTEGER NOT NULL,
    max_momentum_streak         INTEGER NOT NULL,
    blunder_cluster_density     INTEGER NOT NULL,
    blunder_cluster_range_start INTEGER,
    blunder_cluster_range_end   INTEGER,
    time_quality_correlation    REAL,
    avg_blunder_time_ms         INTEGER,
    avg_good_move_time_ms       INTEGER,
    opening_avg_cp_loss         REAL NOT NULL,
    middlegame_avg_cp_loss      REAL NOT NULL,
    endgame_avg_cp_loss         REAL NOT NULL,
    UNIQUE(game_id, color),
    CHECK((blunder_cluster_range_start IS NULL) = (blunder_cluster_range_end IS NULL))
) STRICT;

CREATE TABLE advanced_position_analyses (
    id              INTEGER PRIMARY KEY,
    game_id         TEXT NOT NULL
                    REFERENCES advanced_game_analyses(game_id) ON DELETE CASCADE,
    ply             INTEGER NOT NULL,
    is_critical     INTEGER NOT NULL CHECK(is_critical IN (0, 1)),
    deep_depth      INTEGER,
    tension_mutually_attacked_pairs  INTEGER NOT NULL,
    tension_contested_squares        INTEGER NOT NULL,
    tension_attacked_but_defended    INTEGER NOT NULL,
    tension_forcing_moves            INTEGER NOT NULL,
    tension_checks_available         INTEGER NOT NULL,
    tension_captures_available       INTEGER NOT NULL,
    tension_volatility_score         REAL NOT NULL CHECK(tension_volatility_score >= 0.0),
    ks_white_pawn_shield_count       INTEGER NOT NULL,
    ks_white_open_files_near_king    INTEGER NOT NULL,
    ks_white_attacker_count          INTEGER NOT NULL,
    ks_white_attack_weight           INTEGER NOT NULL,
    ks_white_attacked_king_zone_sq   INTEGER NOT NULL,
    ks_white_king_zone_size          INTEGER NOT NULL,
    ks_white_exposure_score          REAL NOT NULL CHECK(ks_white_exposure_score >= 0.0),
    ks_black_pawn_shield_count       INTEGER NOT NULL,
    ks_black_open_files_near_king    INTEGER NOT NULL,
    ks_black_attacker_count          INTEGER NOT NULL,
    ks_black_attack_weight           INTEGER NOT NULL,
    ks_black_attacked_king_zone_sq   INTEGER NOT NULL,
    ks_black_king_zone_size          INTEGER NOT NULL,
    ks_black_exposure_score          REAL NOT NULL CHECK(ks_black_exposure_score >= 0.0),
    tactics_before_fork_count              INTEGER NOT NULL,
    tactics_before_pin_count               INTEGER NOT NULL,
    tactics_before_skewer_count            INTEGER NOT NULL,
    tactics_before_discovered_attack_count INTEGER NOT NULL,
    tactics_before_hanging_piece_count     INTEGER NOT NULL,
    tactics_before_has_back_rank_weakness  INTEGER NOT NULL CHECK(tactics_before_has_back_rank_weakness IN (0, 1)),
    tactics_before_patterns                TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_before_patterns)),
    tactics_after_fork_count               INTEGER NOT NULL,
    tactics_after_pin_count                INTEGER NOT NULL,
    tactics_after_skewer_count             INTEGER NOT NULL,
    tactics_after_discovered_attack_count  INTEGER NOT NULL,
    tactics_after_hanging_piece_count      INTEGER NOT NULL,
    tactics_after_has_back_rank_weakness   INTEGER NOT NULL CHECK(tactics_after_has_back_rank_weakness IN (0, 1)),
    tactics_after_patterns                 TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_after_patterns)),
    UNIQUE(game_id, ply)
) STRICT;
