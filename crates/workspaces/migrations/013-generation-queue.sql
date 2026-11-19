CREATE TABLE generation_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    draft_id INTEGER NOT NULL,
    request_key TEXT NOT NULL CHECK (length(request_key) > 0),
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'running', 'paused', 'failed', 'completed', 'cancelled')),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    UNIQUE (workspace_id, request_key),
    FOREIGN KEY (workspace_id, draft_id) REFERENCES video_drafts(workspace_id, id)
);

CREATE INDEX generation_runs_by_draft ON generation_runs(workspace_id, draft_id);

PRAGMA user_version = 13;
