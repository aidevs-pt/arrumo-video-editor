-- Where an agent's CLI is installed when it isn't on the PATH, per agent plugin. It belongs to
-- this computer, so backups leave it out.
CREATE TABLE agent_executables (
    plugin_id TEXT PRIMARY KEY CHECK (length(plugin_id) BETWEEN 1 AND 200),
    path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 4096)
);

PRAGMA user_version = 67;
