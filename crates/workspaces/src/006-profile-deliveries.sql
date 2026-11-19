CREATE TABLE profile_deliveries (
    workspace_id INTEGER NOT NULL,
    profile_id INTEGER NOT NULL,
    format_id INTEGER NOT NULL,
    settings_json TEXT NOT NULL CHECK (json_valid(settings_json)),
    PRIMARY KEY (workspace_id, profile_id, format_id),
    FOREIGN KEY (workspace_id, profile_id) REFERENCES profiles(workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, format_id) REFERENCES output_formats(workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX profile_deliveries_by_format ON profile_deliveries(workspace_id, format_id);

PRAGMA user_version = 6;
