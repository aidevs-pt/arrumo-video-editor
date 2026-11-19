CREATE TABLE preview_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    max_edge INTEGER CHECK (max_edge BETWEEN 144 AND 2160)
);

PRAGMA user_version = 18;
