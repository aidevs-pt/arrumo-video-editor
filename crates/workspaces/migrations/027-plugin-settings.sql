CREATE TABLE plugin_setting_values (
    plugin_id TEXT NOT NULL,
    setting_id TEXT NOT NULL,
    workspace_id INTEGER REFERENCES workspaces(id) ON DELETE CASCADE,
    value TEXT NOT NULL CHECK (json_valid(value) AND json_type(value) IN ('text', 'true', 'false', 'integer'))
) STRICT;

CREATE UNIQUE INDEX plugin_setting_scope
ON plugin_setting_values(plugin_id, setting_id, coalesce(workspace_id, 0));

CREATE TRIGGER remove_uninstalled_plugin_settings
AFTER DELETE ON installed_plugin_manifests
BEGIN
    DELETE FROM plugin_setting_values WHERE plugin_id = OLD.plugin_id;
END;

CREATE TRIGGER remove_incompatible_plugin_settings
AFTER UPDATE OF manifest ON installed_plugin_manifests
BEGIN
    DELETE FROM plugin_setting_values
    WHERE plugin_id = NEW.plugin_id AND NOT EXISTS (
        SELECT 1 FROM json_each(CAST(NEW.manifest AS TEXT), '$.settings') AS setting
        WHERE json_extract(setting.value, '$.id') = setting_id
          AND (workspace_id IS NULL OR json_extract(setting.value, '$.scope') = 'workspace')
          AND json_extract(setting.value, '$.kind') = CASE json_type(plugin_setting_values.value)
              WHEN 'text' THEN 'string'
              WHEN 'integer' THEN 'integer'
              ELSE 'boolean'
          END
    );
END;

PRAGMA user_version = 27;
