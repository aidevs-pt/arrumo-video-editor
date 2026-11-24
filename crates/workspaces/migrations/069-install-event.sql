-- Usage counts a new install once instead of every launch. Existing installs were already
-- counted; one that has no workspace yet is treated as new.
ALTER TABLE privacy_settings ADD COLUMN install_reported INTEGER NOT NULL DEFAULT 1;
UPDATE privacy_settings SET install_reported = 0 WHERE NOT EXISTS (SELECT 1 FROM workspaces);

PRAGMA user_version = 69;
