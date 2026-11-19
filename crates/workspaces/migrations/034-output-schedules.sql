CREATE TABLE output_schedules (
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    format_id INTEGER NOT NULL,
    scheduled_date TEXT NOT NULL,
    PRIMARY KEY (run_id, format_id)
) STRICT;
CREATE INDEX output_schedules_by_date ON output_schedules(scheduled_date, run_id);

PRAGMA user_version = 34;
