-- Drop legacy tactical count and pattern columns.
-- These are replaced by the JSON tactics_before_tags / tactics_after_tags columns
-- added in migration 002.

-- SQLite doesn't support DROP COLUMN before 3.35.0, so we recreate the table.
-- Step 1: Create new table without legacy columns
CREATE TABLE advanced_position_analyses_new (
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
    tactics_before_tags              TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_before_tags)),
    tactics_after_tags               TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_after_tags)),
    UNIQUE(game_id, ply)
) STRICT;

-- Step 2: Copy data from old table
INSERT INTO advanced_position_analyses_new
    (id, game_id, ply, is_critical, deep_depth,
     tension_mutually_attacked_pairs, tension_contested_squares,
     tension_attacked_but_defended, tension_forcing_moves,
     tension_checks_available, tension_captures_available,
     tension_volatility_score,
     ks_white_pawn_shield_count, ks_white_open_files_near_king,
     ks_white_attacker_count, ks_white_attack_weight,
     ks_white_attacked_king_zone_sq, ks_white_king_zone_size,
     ks_white_exposure_score,
     ks_black_pawn_shield_count, ks_black_open_files_near_king,
     ks_black_attacker_count, ks_black_attack_weight,
     ks_black_attacked_king_zone_sq, ks_black_king_zone_size,
     ks_black_exposure_score,
     tactics_before_tags, tactics_after_tags)
SELECT id, game_id, ply, is_critical, deep_depth,
       tension_mutually_attacked_pairs, tension_contested_squares,
       tension_attacked_but_defended, tension_forcing_moves,
       tension_checks_available, tension_captures_available,
       tension_volatility_score,
       ks_white_pawn_shield_count, ks_white_open_files_near_king,
       ks_white_attacker_count, ks_white_attack_weight,
       ks_white_attacked_king_zone_sq, ks_white_king_zone_size,
       ks_white_exposure_score,
       ks_black_pawn_shield_count, ks_black_open_files_near_king,
       ks_black_attacker_count, ks_black_attack_weight,
       ks_black_attacked_king_zone_sq, ks_black_king_zone_size,
       ks_black_exposure_score,
       tactics_before_tags, tactics_after_tags
FROM advanced_position_analyses;

-- Step 3: Drop old table and rename new one
DROP TABLE advanced_position_analyses;
ALTER TABLE advanced_position_analyses_new RENAME TO advanced_position_analyses;
