CREATE TABLE workspace_assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) > 0 AND instr(name, char(0)) = 0),
    kind TEXT NOT NULL CHECK (kind IN ('image', 'audio', 'video')),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    notes TEXT NOT NULL DEFAULT ''
);

CREATE INDEX workspace_assets_scope ON workspace_assets(workspace_id, kind, id);

PRAGMA user_version = 7;
