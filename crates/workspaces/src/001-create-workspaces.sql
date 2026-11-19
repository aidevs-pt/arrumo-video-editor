CREATE TABLE workspaces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(name) > 0 AND instr(name, char(0)) = 0)
);

PRAGMA user_version = 1;
