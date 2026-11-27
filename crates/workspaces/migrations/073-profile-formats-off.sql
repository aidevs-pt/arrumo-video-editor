-- A profile may leave out formats its workspace makes, such as a profile that only makes
-- portrait videos. Its videos are then made in the formats it keeps.
CREATE TABLE profile_formats_off (
    workspace_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    format_id INTEGER NOT NULL,
    PRIMARY KEY (workspace_id, profile_id, format_id),
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, format_id) REFERENCES output_formats(workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX profile_formats_off_by_format ON profile_formats_off(workspace_id, format_id);

PRAGMA user_version = 73;
