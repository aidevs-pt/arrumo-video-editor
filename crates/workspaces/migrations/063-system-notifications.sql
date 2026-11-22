DROP TRIGGER generation_notification_transition;

CREATE TABLE generation_notifications_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('review_required', 'completed', 'failed')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    read_at INTEGER
);
INSERT INTO generation_notifications_v2 (id, run_id, kind, created_at, read_at)
SELECT id, run_id, kind, created_at, read_at FROM generation_notifications;
DROP TABLE generation_notifications;
ALTER TABLE generation_notifications_v2 RENAME TO generation_notifications;
CREATE INDEX generation_notifications_by_run ON generation_notifications(run_id, id);

CREATE TRIGGER generation_notification_transition
AFTER UPDATE OF state, pending_review_id ON generation_runs
WHEN (NEW.state = 'completed' AND OLD.state != 'completed')
  OR (NEW.state = 'failed' AND OLD.state != 'failed')
  OR (NEW.state = 'paused' AND NEW.pending_review_id IS NOT NULL
      AND NEW.pending_review_id IS NOT OLD.pending_review_id)
BEGIN
    INSERT INTO generation_notifications (run_id, kind)
    VALUES (NEW.id, CASE NEW.state WHEN 'completed' THEN 'completed' WHEN 'failed' THEN 'failed' ELSE 'review_required' END);
END;

CREATE TABLE notification_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    system INTEGER NOT NULL DEFAULT 1 CHECK (system IN (0, 1))
);
INSERT INTO notification_settings (id) VALUES (1);

PRAGMA user_version = 63;
