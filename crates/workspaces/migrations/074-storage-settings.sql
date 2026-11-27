-- Whether clips uploaded for a video are deleted once it is finished. Missing means yes.
CREATE TABLE storage_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    delete_used_footage INTEGER NOT NULL DEFAULT 1 CHECK (delete_used_footage IN (0, 1))
);

PRAGMA user_version = 74;
