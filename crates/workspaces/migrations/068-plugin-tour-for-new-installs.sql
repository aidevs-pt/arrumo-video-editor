-- The tour now introduces the bundled plugins too, once the first workspace exists. Installs
-- that marked them introduced without showing anything get the tour once.
DELETE FROM plugin_introductions_seen;

PRAGMA user_version = 68;
