CREATE TABLE format_deliveries (
    format_id INTEGER PRIMARY KEY REFERENCES output_formats(id) ON DELETE CASCADE,
    settings_json TEXT NOT NULL CHECK (json_valid(settings_json))
);

PRAGMA user_version = 5;
