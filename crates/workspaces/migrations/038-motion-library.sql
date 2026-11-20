CREATE TABLE motion_library (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    profile_id INTEGER,
    kind TEXT NOT NULL CHECK (kind IN ('animation', 'transition')),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    usage_markdown TEXT NOT NULL,
    preview_asset_id INTEGER NOT NULL,
    thumbnail_asset_id INTEGER,
    sound_asset_id INTEGER,
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, preview_asset_id) REFERENCES workspace_assets(workspace_id, id),
    FOREIGN KEY (workspace_id, thumbnail_asset_id) REFERENCES workspace_assets(workspace_id, id),
    FOREIGN KEY (workspace_id, sound_asset_id) REFERENCES workspace_assets(workspace_id, id)
) STRICT;

CREATE INDEX motion_library_scope ON motion_library(workspace_id, profile_id, id);

PRAGMA user_version = 38;
