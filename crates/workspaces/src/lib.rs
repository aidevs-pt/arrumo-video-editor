use std::{path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
}

pub struct WorkspaceStore(Connection);

impl WorkspaceStore {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let migration = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = migration.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => migration.execute_batch(include_str!("001-create-workspaces.sql"))?,
            1 => {}
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
        migration.commit()?;
        Ok(Self(connection))
    }

    pub fn create(&self, name: &str) -> rusqlite::Result<Workspace> {
        let name = name.trim();
        self.0.query_row(
            "INSERT INTO workspaces (name) VALUES (?1) RETURNING id, name",
            [name],
            Self::read_workspace,
        )
    }

    pub fn list(&self) -> rusqlite::Result<Vec<Workspace>> {
        self.0
            .prepare("SELECT id, name FROM workspaces ORDER BY id")?
            .query_map([], Self::read_workspace)?
            .collect()
    }

    fn read_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
        Ok(Workspace {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_lists_workspaces() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        assert!(store.list().unwrap().is_empty());
        let workspace = store.create("  Studio  ").unwrap();
        assert_eq!(workspace.name, "Studio");
        assert_eq!(store.list().unwrap(), vec![workspace]);
    }

    #[test]
    fn rejects_blank_names_without_creating_a_record() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        assert!(store.create(" \t ").is_err());
        assert!(store.list().unwrap().is_empty());
    }
}
