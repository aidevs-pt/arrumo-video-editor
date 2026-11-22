CREATE TABLE plugin_tour_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    -- Plugins present before the tour existed are treated as already introduced.
    baseline_taken INTEGER NOT NULL DEFAULT 0 CHECK (baseline_taken IN (0, 1))
);

INSERT INTO plugin_tour_state (id) VALUES (1);

CREATE TABLE plugin_introductions_seen (
    plugin_id TEXT PRIMARY KEY CHECK (length(plugin_id) BETWEEN 1 AND 128),
    seen_at INTEGER NOT NULL DEFAULT (unixepoch())
);

PRAGMA user_version = 58;
