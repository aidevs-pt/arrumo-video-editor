CREATE UNIQUE INDEX production_result_run_id ON production_results (run_id, id);

CREATE TABLE completed_video_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    result_id INTEGER,
    markdown TEXT NOT NULL CHECK (length(trim(markdown)) > 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (run_id, result_id) REFERENCES production_results(run_id, id) ON DELETE CASCADE
);

CREATE INDEX completed_video_feedback_by_run ON completed_video_feedback (run_id, id);

PRAGMA user_version = 21;
