CREATE TABLE profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) > 0 AND instr(name, char(0)) = 0),
    guidance TEXT NOT NULL DEFAULT '',
    color TEXT CHECK (
        color IS NULL OR (
            length(color) = 7 AND substr(color, 1, 1) = '#'
            AND substr(color, 2) NOT GLOB '*[^0-9A-Fa-f]*'
        )
    )
);

CREATE INDEX profiles_by_workspace ON profiles(workspace_id, id);

PRAGMA user_version = 2;
