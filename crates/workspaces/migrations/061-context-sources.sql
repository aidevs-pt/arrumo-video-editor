CREATE TABLE workspace_context_sources (
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 2048),
    readable INTEGER NOT NULL CHECK (readable IN (0, 1)),
    read_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (workspace_id, source)
) STRICT;

PRAGMA user_version = 61;
