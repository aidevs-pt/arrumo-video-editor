-- Earlier states of library animations, so a change can be previewed against and undone.
ALTER TABLE motion_library ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE motion_library ADD COLUMN change_note TEXT NOT NULL DEFAULT '';
ALTER TABLE motion_library ADD COLUMN restored_from INTEGER;

CREATE TABLE motion_library_versions (
    workspace_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    version INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('animation', 'transition')),
    title TEXT NOT NULL,
    usage_markdown TEXT NOT NULL,
    preview_asset_id INTEGER NOT NULL,
    thumbnail_asset_id INTEGER,
    sound_asset_id INTEGER,
    spec_json TEXT,
    change_note TEXT NOT NULL DEFAULT '',
    restored_from INTEGER,
    replaced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (workspace_id, item_id, version),
    FOREIGN KEY (workspace_id, item_id) REFERENCES motion_library(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, preview_asset_id) REFERENCES workspace_assets(workspace_id, id),
    FOREIGN KEY (workspace_id, thumbnail_asset_id) REFERENCES workspace_assets(workspace_id, id),
    FOREIGN KEY (workspace_id, sound_asset_id) REFERENCES workspace_assets(workspace_id, id)
) STRICT;

PRAGMA user_version = 75;
