ALTER TABLE generation_runs ADD COLUMN replay_of_run_id INTEGER REFERENCES generation_runs(id);
ALTER TABLE generation_runs ADD COLUMN replay_mode TEXT CHECK (
    (replay_of_run_id IS NULL AND replay_mode IS NULL)
    OR (replay_of_run_id IS NOT NULL AND replay_mode IS NOT NULL AND replay_mode IN ('captured', 'current'))
);

PRAGMA user_version = 20;
