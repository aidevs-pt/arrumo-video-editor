-- Crash reports carry their own random ID, separate from the usage install ID, so reports from
-- one install can be grouped without linking them to usage data.
ALTER TABLE privacy_settings ADD COLUMN crash_report_id TEXT
    CHECK (crash_report_id IS NULL OR length(crash_report_id) = 32);

UPDATE privacy_settings SET crash_report_id = lower(hex(randomblob(16)));

PRAGMA user_version = 72;
