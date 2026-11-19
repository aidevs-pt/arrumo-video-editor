CREATE TABLE generation_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('review_required', 'completed')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX generation_notifications_by_run ON generation_notifications(run_id, id);

INSERT INTO generation_notifications (run_id, kind)
SELECT id, CASE WHEN state = 'completed' THEN 'completed' ELSE 'review_required' END
FROM generation_runs
WHERE state = 'completed' OR (state = 'paused' AND pending_review_id IS NOT NULL);

CREATE TRIGGER generation_notification_transition
AFTER UPDATE OF state, pending_review_id ON generation_runs
WHEN (NEW.state = 'completed' AND OLD.state != 'completed')
  OR (NEW.state = 'paused' AND NEW.pending_review_id IS NOT NULL
      AND NEW.pending_review_id IS NOT OLD.pending_review_id)
BEGIN
    INSERT INTO generation_notifications (run_id, kind)
    VALUES (NEW.id, CASE WHEN NEW.state = 'completed' THEN 'completed' ELSE 'review_required' END);
END;

PRAGMA user_version = 31;
