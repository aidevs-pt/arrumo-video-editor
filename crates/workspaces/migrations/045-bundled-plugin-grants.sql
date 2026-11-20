DROP TRIGGER revoke_plugin_grants_on_update;
ALTER TABLE plugin_grants RENAME TO installed_plugin_grants;

CREATE TABLE plugin_grants (
    plugin_id TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    permission TEXT NOT NULL CHECK (permission = 'workspace-context-read'),
    PRIMARY KEY (plugin_id, workspace_id, permission)
) STRICT;

INSERT INTO plugin_grants SELECT * FROM installed_plugin_grants;
DROP TABLE installed_plugin_grants;

CREATE TRIGGER revoke_plugin_grants_on_update
AFTER UPDATE OF package_sha256, release_source ON installed_plugin_manifests
WHEN NEW.package_sha256 IS NOT OLD.package_sha256 OR NEW.release_source IS NOT OLD.release_source
BEGIN
    DELETE FROM plugin_grants WHERE plugin_id = OLD.plugin_id;
END;

CREATE TRIGGER revoke_plugin_grants_on_remove
AFTER DELETE ON installed_plugin_manifests
BEGIN
    DELETE FROM plugin_grants WHERE plugin_id = OLD.plugin_id;
END;

PRAGMA user_version = 45;
