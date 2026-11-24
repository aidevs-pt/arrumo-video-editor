-- Usage counts an install once instead of every launch. Databases that already exist belong
-- to installs being updated; the store marks one it creates from scratch as not yet counted.
ALTER TABLE privacy_settings ADD COLUMN install_reported INTEGER NOT NULL DEFAULT 1;

PRAGMA user_version = 69;
