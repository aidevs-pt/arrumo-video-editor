-- A workspace's heading and body fonts. Files live under the workspace's media folder, named by
-- their SHA-256, so runs that snapshotted a font keep rendering with it after it is replaced.
CREATE TABLE brand_fonts (
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('heading', 'body')),
    weight TEXT NOT NULL CHECK (weight IN ('regular', 'bold')),
    family TEXT NOT NULL CHECK (length(family) BETWEEN 1 AND 120),
    file_name TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    advance REAL NOT NULL CHECK (advance > 0 AND advance < 2),
    PRIMARY KEY (workspace_id, role, weight)
);

PRAGMA user_version = 66;
