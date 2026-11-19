ALTER TABLE installed_plugin_manifests ADD COLUMN release_source TEXT
    CHECK (release_source IS NULL OR (package_sha256 IS NOT NULL AND json_valid(release_source)));

PRAGMA user_version = 26;
