CREATE UNIQUE INDEX motion_library_workspace_item ON motion_library(workspace_id, id);

CREATE TABLE motion_profile_shares (
    workspace_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    PRIMARY KEY (workspace_id, item_id, profile_id),
    FOREIGN KEY (workspace_id, item_id) REFERENCES motion_library(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX motion_profile_shares_by_profile ON motion_profile_shares(workspace_id, profile_id, item_id);

PRAGMA user_version = 39;
