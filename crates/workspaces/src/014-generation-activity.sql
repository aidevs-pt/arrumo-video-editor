ALTER TABLE generation_runs ADD COLUMN attempt INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generation_runs ADD COLUMN lease_until INTEGER;
ALTER TABLE generation_runs ADD COLUMN checkpoint_json TEXT CHECK (json_valid(checkpoint_json));

CREATE TABLE generation_activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    attempt INTEGER NOT NULL,
    state TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX generation_activity_by_run ON generation_activity(run_id, id);

INSERT INTO generation_activity (run_id, attempt, state, kind)
SELECT id, attempt, state, 'submitted' FROM generation_runs;

CREATE TRIGGER generation_activity_submission
AFTER INSERT ON generation_runs
BEGIN
    INSERT INTO generation_activity (run_id, attempt, state, kind)
    VALUES (NEW.id, NEW.attempt, NEW.state, 'submitted');
END;

CREATE TRIGGER generation_activity_transition
AFTER UPDATE OF state, attempt ON generation_runs
WHEN NEW.state != OLD.state OR NEW.attempt != OLD.attempt
BEGIN
    INSERT INTO generation_activity (run_id, attempt, state, kind)
    VALUES (NEW.id, NEW.attempt, NEW.state,
            CASE WHEN NEW.attempt != OLD.attempt THEN 'claimed' ELSE 'transition' END);
END;

PRAGMA user_version = 14;
