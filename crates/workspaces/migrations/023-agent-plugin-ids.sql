CREATE TABLE app_agent_plugins (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    plugin_id TEXT NOT NULL CHECK (length(plugin_id) > 0)
);

INSERT INTO app_agent_plugins (id, plugin_id)
SELECT id, replace(provider, '_', '-') FROM app_agent_settings;
DROP TABLE app_agent_settings;

ALTER TABLE workspaces ADD COLUMN agent_plugin_id TEXT;
UPDATE workspaces SET agent_plugin_id = replace(agent_provider, '_', '-');
ALTER TABLE workspaces DROP COLUMN agent_provider;

PRAGMA user_version = 23;
