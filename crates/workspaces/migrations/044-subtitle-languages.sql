ALTER TABLE subtitle_revisions RENAME TO primary_subtitle_revisions;

CREATE TABLE subtitle_revisions (
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    language TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL CHECK (revision > 0),
    subtitles TEXT NOT NULL,
    cue_count INTEGER NOT NULL CHECK (cue_count > 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (run_id, format_id, language, revision)
);

INSERT INTO subtitle_revisions (run_id, format_id, revision, subtitles, cue_count, created_at)
SELECT run_id, format_id, revision, subtitles, cue_count, created_at
FROM primary_subtitle_revisions;

DROP TABLE primary_subtitle_revisions;

PRAGMA user_version = 44;
