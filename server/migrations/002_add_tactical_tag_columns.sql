ALTER TABLE advanced_position_analyses
ADD COLUMN tactics_before_tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_before_tags));

ALTER TABLE advanced_position_analyses
ADD COLUMN tactics_after_tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tactics_after_tags));

UPDATE advanced_position_analyses
SET tactics_before_tags = '[]',
    tactics_after_tags = '[]'
WHERE tactics_before_tags IS NULL
   OR tactics_after_tags IS NULL;
