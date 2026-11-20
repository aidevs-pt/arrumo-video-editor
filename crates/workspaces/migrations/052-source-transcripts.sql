-- Transcripts of imported footage, reused by every run that uses the same managed file.
CREATE TABLE asset_transcripts (
    workspace_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    provider TEXT NOT NULL,
    language_code TEXT NOT NULL,
    text TEXT NOT NULL,
    words_json TEXT NOT NULL CHECK (json_valid(words_json)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (workspace_id, asset_id),
    FOREIGN KEY (workspace_id, asset_id) REFERENCES workspace_assets(workspace_id, id) ON DELETE CASCADE
) STRICT;

PRAGMA user_version = 52;
