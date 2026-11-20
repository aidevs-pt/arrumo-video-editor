CREATE TABLE asset_organization (
    workspace_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    group_name TEXT,
    labels_json TEXT NOT NULL CHECK (json_valid(labels_json) AND json_type(labels_json) = 'array'),
    PRIMARY KEY (workspace_id, asset_id),
    FOREIGN KEY (workspace_id, asset_id) REFERENCES workspace_assets(workspace_id, id) ON DELETE CASCADE
) STRICT;

PRAGMA user_version = 35;
