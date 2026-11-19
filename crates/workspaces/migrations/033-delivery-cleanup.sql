CREATE TABLE pending_delivery_cleanup (
    workspace_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL,
    confirmed_title TEXT NOT NULL,
    PRIMARY KEY (workspace_id, run_id)
) STRICT;

PRAGMA user_version = 33;
