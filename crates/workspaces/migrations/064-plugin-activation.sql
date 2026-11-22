-- Whether each plugin may run. Without a row, bundled plugins are on and imported ones off;
-- imports installed before this switch existed stay on.
CREATE TABLE plugin_activation (
    plugin_id TEXT NOT NULL PRIMARY KEY,
    active INTEGER NOT NULL CHECK (active IN (0, 1))
) STRICT;
INSERT INTO plugin_activation (plugin_id, active)
SELECT plugin_id, 1 FROM installed_plugin_manifests;

PRAGMA user_version = 64;
