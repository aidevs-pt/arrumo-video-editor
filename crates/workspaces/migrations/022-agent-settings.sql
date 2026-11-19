CREATE TABLE app_agent_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    provider TEXT NOT NULL CHECK (provider IN ('codex', 'claude_code'))
);

ALTER TABLE workspaces ADD COLUMN agent_provider TEXT CHECK (
    agent_provider IN ('codex', 'claude_code')
);

PRAGMA user_version = 22;
