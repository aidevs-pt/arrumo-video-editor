ALTER TABLE generation_runs ADD COLUMN plan_json TEXT CHECK (json_valid(plan_json));

PRAGMA user_version = 16;
