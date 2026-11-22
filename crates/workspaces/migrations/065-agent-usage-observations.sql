-- The latest allowance an agent reported while it worked, for agents that report it only then.
CREATE TABLE agent_usage_observations (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    used_percent REAL NOT NULL CHECK (used_percent BETWEEN 0 AND 100),
    resets_at INTEGER,
    window_minutes INTEGER,
    observed_at INTEGER NOT NULL
) STRICT;

PRAGMA user_version = 65;
