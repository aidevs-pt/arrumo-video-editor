CREATE TABLE plugin_provider_preferences (
    capability TEXT NOT NULL,
    workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
    plugin_ids TEXT NOT NULL CHECK (json_valid(plugin_ids) AND json_type(plugin_ids) = 'array')
) STRICT;

CREATE UNIQUE INDEX app_provider_preferences
ON plugin_provider_preferences(capability) WHERE workspace_id IS NULL;

CREATE UNIQUE INDEX workspace_provider_preferences
ON plugin_provider_preferences(capability, workspace_id) WHERE workspace_id IS NOT NULL;

CREATE TRIGGER remove_uninstalled_provider_preferences
AFTER DELETE ON installed_plugin_manifests
BEGIN
    UPDATE plugin_provider_preferences
    SET plugin_ids = (
        SELECT json_group_array(value) FROM json_each(plugin_ids) WHERE value != OLD.plugin_id
    )
    WHERE EXISTS (SELECT 1 FROM json_each(plugin_ids) WHERE value = OLD.plugin_id);
END;

CREATE TRIGGER remove_unavailable_provider_preferences
AFTER UPDATE OF manifest ON installed_plugin_manifests
BEGIN
    UPDATE plugin_provider_preferences
    SET plugin_ids = (
        SELECT json_group_array(value) FROM json_each(plugin_ids) WHERE value != NEW.plugin_id
    )
    WHERE EXISTS (SELECT 1 FROM json_each(plugin_ids) WHERE value = NEW.plugin_id)
      AND NOT EXISTS (
          SELECT 1 FROM json_each(CAST(NEW.manifest AS TEXT), '$.capabilities')
          WHERE key = capability
      );
END;

PRAGMA user_version = 29;
