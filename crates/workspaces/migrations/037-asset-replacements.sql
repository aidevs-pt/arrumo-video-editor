ALTER TABLE workspace_assets ADD COLUMN superseded INTEGER NOT NULL DEFAULT 0
    CHECK (superseded IN (0, 1));

PRAGMA user_version = 37;
