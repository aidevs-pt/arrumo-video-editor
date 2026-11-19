ALTER TABLE installed_plugin_manifests ADD COLUMN package_bytes BLOB;
ALTER TABLE installed_plugin_manifests ADD COLUMN package_sha256 TEXT
    CHECK ((package_sha256 IS NULL AND package_bytes IS NULL)
        OR (package_sha256 IS NOT NULL AND length(package_sha256) = 64 AND package_bytes IS NOT NULL));

PRAGMA user_version = 25;
