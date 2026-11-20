CREATE TABLE completed_media (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL REFERENCES workspace_assets(id) ON DELETE NO ACTION,
    UNIQUE (run_id, format_id, asset_id)
) STRICT;

CREATE INDEX completed_media_asset ON completed_media(asset_id);

PRAGMA user_version = 46;
