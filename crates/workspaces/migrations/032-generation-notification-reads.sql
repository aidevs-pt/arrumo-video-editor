ALTER TABLE generation_notifications ADD COLUMN read_at INTEGER;

PRAGMA user_version = 32;
