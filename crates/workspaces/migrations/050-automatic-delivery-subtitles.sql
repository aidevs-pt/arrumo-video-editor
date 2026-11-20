CREATE TABLE automatic_delivery_subtitles (
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'started' CHECK(state IN ('started', 'completed', 'failed')),
    PRIMARY KEY (run_id, format_id)
) STRICT;

PRAGMA user_version = 50;
