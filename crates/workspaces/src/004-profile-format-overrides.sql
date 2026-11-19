CREATE UNIQUE INDEX profiles_workspace_and_id ON profiles(workspace_id, id);
CREATE UNIQUE INDEX formats_workspace_and_id ON output_formats(workspace_id, id);

CREATE TABLE profile_format_overrides (
    workspace_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    format_id INTEGER NOT NULL,
    width INTEGER CHECK (width BETWEEN 1 AND 4294967295),
    height INTEGER CHECK (height BETWEEN 1 AND 4294967295),
    PRIMARY KEY (workspace_id, profile_id, format_id),
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, format_id) REFERENCES output_formats(workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX profile_overrides_by_format ON profile_format_overrides(workspace_id, format_id);

PRAGMA user_version = 4;
