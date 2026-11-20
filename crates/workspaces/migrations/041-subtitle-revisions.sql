CREATE TABLE subtitle_revisions (
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    subtitles TEXT NOT NULL,
    cue_count INTEGER NOT NULL CHECK (cue_count > 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (run_id, format_id, revision)
);

PRAGMA user_version = 41;
