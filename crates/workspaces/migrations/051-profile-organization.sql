ALTER TABLE profiles ADD COLUMN description TEXT NOT NULL DEFAULT ''
    CHECK (length(description) <= 280 AND instr(description, char(0)) = 0);

CREATE TABLE profile_format_context (
    workspace_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    format_id INTEGER NOT NULL,
    markdown TEXT NOT NULL,
    PRIMARY KEY (workspace_id, profile_id, format_id),
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, format_id) REFERENCES output_formats(workspace_id, id) ON DELETE CASCADE
) STRICT;

-- A share without a format applies to every format of the profile.
CREATE TABLE asset_profile_shares (
    workspace_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    format_id INTEGER,
    FOREIGN KEY (workspace_id, asset_id) REFERENCES workspace_assets(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, format_id) REFERENCES output_formats(workspace_id, id) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX asset_profile_shares_unique
    ON asset_profile_shares(workspace_id, profile_id, asset_id, ifnull(format_id, 0));
CREATE INDEX asset_profile_shares_by_asset ON asset_profile_shares(workspace_id, asset_id);

PRAGMA user_version = 51;
