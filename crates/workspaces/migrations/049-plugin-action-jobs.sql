CREATE TABLE plugin_action_jobs (
    id TEXT PRIMARY KEY,
    workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    package_sha256 TEXT NOT NULL CHECK(length(package_sha256) = 64),
    input TEXT NOT NULL CHECK(json_valid(input)),
    state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued', 'running', 'completed', 'failed')),
    attempt INTEGER NOT NULL DEFAULT 0,
    lease_until INTEGER,
    result TEXT CHECK(result IS NULL OR json_valid(result)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

PRAGMA user_version = 49;
