ALTER TABLE completed_media RENAME TO previous_completed_media;

CREATE TABLE completed_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL REFERENCES workspace_assets(id) ON DELETE NO ACTION,
    UNIQUE (run_id, format_id, asset_id)
) STRICT;

INSERT INTO completed_media SELECT * FROM previous_completed_media;
DROP TABLE previous_completed_media;
CREATE INDEX completed_media_asset ON completed_media(asset_id);

PRAGMA user_version = 47;
