CREATE TABLE plugin_grants (
    plugin_id TEXT NOT NULL REFERENCES installed_plugin_manifests(plugin_id) ON DELETE CASCADE,
    package_sha256 TEXT NOT NULL,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    permission TEXT NOT NULL CHECK (permission = 'workspace-context-read'),
    PRIMARY KEY (plugin_id, workspace_id, permission)
) STRICT;

CREATE TRIGGER revoke_plugin_grants_on_update
AFTER UPDATE OF package_sha256, release_source ON installed_plugin_manifests
WHEN NEW.package_sha256 IS NOT OLD.package_sha256 OR NEW.release_source IS NOT OLD.release_source
BEGIN
    DELETE FROM plugin_grants WHERE plugin_id = OLD.plugin_id;
END;

PRAGMA user_version = 30;
