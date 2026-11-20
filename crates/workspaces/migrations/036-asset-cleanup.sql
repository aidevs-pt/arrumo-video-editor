CREATE TABLE pending_asset_cleanup (
    workspace_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    confirmed_name TEXT NOT NULL,
    PRIMARY KEY (workspace_id, asset_id)
) STRICT;

PRAGMA user_version = 36;
