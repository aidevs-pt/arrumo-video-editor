CREATE TABLE video_drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    profile_id INTEGER,
    title TEXT NOT NULL CHECK (length(title) > 0),
    intent TEXT NOT NULL,
    UNIQUE (workspace_id, id),
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id)
);

CREATE TABLE video_draft_sources (
    workspace_id INTEGER NOT NULL,
    draft_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (workspace_id, draft_id, position),
    UNIQUE (workspace_id, draft_id, asset_id),
    FOREIGN KEY (workspace_id, draft_id) REFERENCES video_drafts(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, asset_id) REFERENCES workspace_assets(workspace_id, id)
);

CREATE INDEX video_draft_sources_by_asset ON video_draft_sources(workspace_id, asset_id);

PRAGMA user_version = 12;
