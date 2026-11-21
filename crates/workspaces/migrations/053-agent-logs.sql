-- What each agent run was asked and printed, so creators can see why a step failed.
CREATE TABLE agent_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id INTEGER REFERENCES generation_runs(id) ON DELETE CASCADE,
    stage TEXT NOT NULL,
    attempt INTEGER,
    provider TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'succeeded', 'failed', 'timed_out', 'unavailable')),
    started_at INTEGER NOT NULL DEFAULT (unixepoch()),
    duration_ms INTEGER,
    exit_code INTEGER,
    command TEXT NOT NULL DEFAULT '',
    prompt TEXT NOT NULL,
    output TEXT NOT NULL DEFAULT '',
    errors TEXT NOT NULL DEFAULT '',
    answer TEXT,
    problem TEXT
) STRICT;

CREATE INDEX agent_logs_scope ON agent_logs(workspace_id, run_id, id);

PRAGMA user_version = 53;
