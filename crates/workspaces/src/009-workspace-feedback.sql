CREATE TABLE workspace_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(title) > 0 AND instr(title, char(0)) = 0),
    markdown TEXT NOT NULL CHECK (length(trim(markdown)) > 0)
);

CREATE INDEX workspace_feedback_scope ON workspace_feedback(workspace_id, id);

PRAGMA user_version = 9;
