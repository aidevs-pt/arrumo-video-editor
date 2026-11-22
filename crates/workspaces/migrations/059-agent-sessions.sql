-- Agent sessions Arrumo kept for follow-ups, removed once nothing can need them.
CREATE TABLE agent_sessions (
    provider TEXT NOT NULL,
    session TEXT NOT NULL,
    workspace_id INTEGER NOT NULL,
    run_id INTEGER,
    stage TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (provider, session)
) STRICT;

PRAGMA user_version = 59;
