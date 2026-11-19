CREATE TABLE output_formats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) > 0 AND instr(name, char(0)) = 0),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    UNIQUE (workspace_id, name)
);

INSERT INTO output_formats (workspace_id, name, width, height, enabled)
SELECT id, 'Landscape', 1920, 1080, 1 FROM workspaces
UNION ALL
SELECT id, 'Portrait', 1080, 1920, 1 FROM workspaces;

CREATE TRIGGER workspaces_seed_output_formats
AFTER INSERT ON workspaces
BEGIN
    INSERT INTO output_formats (workspace_id, name, width, height, enabled)
    VALUES (NEW.id, 'Landscape', 1920, 1080, 1),
           (NEW.id, 'Portrait', 1080, 1920, 1);
END;

PRAGMA user_version = 3;
