ALTER TABLE generation_runs ADD COLUMN stage_index INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generation_runs ADD COLUMN pending_review_id INTEGER;

CREATE TABLE production_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    stage_index INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    result_json TEXT NOT NULL CHECK (json_valid(result_json)),
    decision TEXT CHECK (decision IN ('approved', 'rejected', 'revise')),
    review_notes TEXT
);
CREATE INDEX production_results_by_run ON production_results(run_id, id);

PRAGMA user_version = 17;
