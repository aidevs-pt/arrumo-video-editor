-- A run's round is everything its agent does until the creator sends feedback. Agent logs and
-- kept sessions record their round, so one session serves a whole round.
ALTER TABLE agent_logs ADD COLUMN round INTEGER;
ALTER TABLE agent_sessions ADD COLUMN round INTEGER;

-- Activity names the step a claim started, and tells a claim that picks up an interrupted
-- attempt from one that starts a step.
ALTER TABLE generation_activity ADD COLUMN stage TEXT;

DROP TRIGGER IF EXISTS generation_activity_transition;
CREATE TRIGGER generation_activity_transition
AFTER UPDATE OF state, attempt ON generation_runs
WHEN NEW.state != OLD.state OR NEW.attempt != OLD.attempt
BEGIN
    INSERT INTO generation_activity (run_id, attempt, state, kind, stage)
    VALUES (NEW.id, NEW.attempt, NEW.state,
            CASE
                WHEN NEW.attempt != OLD.attempt AND OLD.state = 'running' THEN 'resumed'
                WHEN NEW.attempt != OLD.attempt THEN 'claimed'
                ELSE 'transition'
            END,
            json_extract(NEW.plan_json, '$.steps[' || NEW.stage_index || '].stage'));
END;

PRAGMA user_version = 71;
