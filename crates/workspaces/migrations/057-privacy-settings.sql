CREATE TABLE privacy_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    usage_data INTEGER NOT NULL DEFAULT 1 CHECK (usage_data IN (0, 1)),
    crash_reports INTEGER NOT NULL DEFAULT 1 CHECK (crash_reports IN (0, 1)),
    install_id TEXT NOT NULL CHECK (length(install_id) = 32)
);

INSERT INTO privacy_settings (id, install_id) VALUES (1, lower(hex(randomblob(16))));

PRAGMA user_version = 57;
