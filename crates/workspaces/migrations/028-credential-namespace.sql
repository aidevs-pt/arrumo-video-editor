CREATE TABLE credential_namespace (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    namespace TEXT NOT NULL CHECK (length(namespace) = 32)
) STRICT;

INSERT INTO credential_namespace (id, namespace) VALUES (1, lower(hex(randomblob(16))));

PRAGMA user_version = 28;
