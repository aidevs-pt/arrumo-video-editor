CREATE TABLE installed_plugin_manifests (
    plugin_id TEXT NOT NULL PRIMARY KEY,
    manifest BLOB NOT NULL
) STRICT;

PRAGMA user_version = 24;
