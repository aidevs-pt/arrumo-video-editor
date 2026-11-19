use std::{path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

mod format_deliveries;
mod output_formats;
mod profile_deliveries;
mod profile_formats;

pub use format_deliveries::{FormatDelivery, RequiredOutput, SupplementalOutputs};
pub use output_formats::{OutputFormat, OutputFormatSettings};
pub use profile_deliveries::{DeliveryOverrides, ProfileDelivery};
pub use profile_formats::{DimensionOverrides, ProfileFormat};

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub guidance: String,
    pub color: Option<String>,
}

pub struct WorkspaceStore(Connection);

impl WorkspaceStore {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let migration = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = migration.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let migrations = [
            include_str!("001-create-workspaces.sql"),
            include_str!("002-create-profiles.sql"),
            include_str!("003-create-output-formats.sql"),
            include_str!("004-profile-format-overrides.sql"),
            include_str!("005-format-deliveries.sql"),
            include_str!("006-profile-deliveries.sql"),
        ];
        let pending = usize::try_from(version)
            .ok()
            .and_then(|version| migrations.get(version..))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        for migration_sql in pending {
            migration.execute_batch(migration_sql)?;
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

    pub fn create_profile(
        &self,
        workspace_id: i64,
        name: &str,
        guidance: &str,
        color: Option<&str>,
    ) -> rusqlite::Result<Profile> {
        self.0.query_row(
            "INSERT INTO profiles (workspace_id, name, guidance, color)
             VALUES (?1, ?2, ?3, ?4) RETURNING id, workspace_id, name, guidance, color",
            rusqlite::params![workspace_id, name.trim(), guidance, color],
            Self::read_profile,
        )
    }

    pub fn update_profile(
        &self,
        workspace_id: i64,
        profile_id: i64,
        name: &str,
        guidance: &str,
        color: Option<&str>,
    ) -> rusqlite::Result<Profile> {
        self.0.query_row(
            "UPDATE profiles SET name = ?3, guidance = ?4, color = ?5
             WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, name, guidance, color",
            rusqlite::params![workspace_id, profile_id, name.trim(), guidance, color],
            Self::read_profile,
        )
    }

    pub fn list_profiles(&self, workspace_id: i64) -> rusqlite::Result<Vec<Profile>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, guidance, color FROM profiles
                 WHERE workspace_id = ?1 ORDER BY id",
            )?
            .query_map([workspace_id], Self::read_profile)?
            .collect()
    }

    fn read_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
        Ok(Profile {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            guidance: row.get(3)?,
            color: row.get(4)?,
        })
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

    #[test]
    fn migrates_an_existing_workspace_database_and_scopes_profiles() {
        let path = std::env::temp_dir().join(format!(
            "arrumo-profile-migration-{}.sqlite",
            std::process::id()
        ));
        let old = Connection::open(&path).unwrap();
        old.execute_batch(include_str!("001-create-workspaces.sql"))
            .unwrap();
        old.execute(
            "INSERT INTO workspaces (name) VALUES ('Existing studio')",
            [],
        )
        .unwrap();
        drop(old);

        let store = WorkspaceStore::open(&path).unwrap();
        let existing = &store.list().unwrap()[0];
        assert_eq!(existing.name, "Existing studio");
        assert_eq!(store.list_output_formats(existing.id).unwrap().len(), 2);
        let other = store.create("Other studio").unwrap();
        let profile = store
            .create_profile(existing.id, "  Reviews  ", "## Keep demos", Some("#532A9D"))
            .unwrap();
        assert_eq!(profile.name, "Reviews");
        assert_eq!(profile.guidance, "## Keep demos");
        assert_eq!(profile.color.as_deref(), Some("#532A9D"));
        assert!(store.list_profiles(other.id).unwrap().is_empty());
        assert!(
            store
                .create_profile(other.id + 1, "Orphan", "", None)
                .is_err()
        );
        assert!(
            store
                .create_profile(other.id, "Bad color", "", Some("#nope00"))
                .is_err()
        );
        drop(store);

        let reopened = WorkspaceStore::open(&path).unwrap();
        assert_eq!(reopened.list_profiles(existing.id).unwrap(), vec![profile]);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn updates_only_the_profile_in_the_requested_workspace() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let first = store.create("First").unwrap();
        let second = store.create("Second").unwrap();
        let profile = store
            .create_profile(first.id, "Reviews", "# Notes", Some("#532A9D"))
            .unwrap();
        assert!(matches!(
            store.update_profile(second.id, profile.id, "Wrong", "", None),
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
        assert!(
            store
                .update_profile(first.id, profile.id, " ", "", None)
                .is_err()
        );
        let profile_id = profile.id;
        assert_eq!(store.list_profiles(first.id).unwrap(), vec![profile]);
        let updated = store
            .update_profile(first.id, profile_id, "  Tutorials ", "## New", None)
            .unwrap();
        assert_eq!(updated.id, profile_id);
        assert_eq!(updated.name, "Tutorials");
        assert_eq!(updated.guidance, "## New");
        assert_eq!(updated.color, None);
        assert_eq!(store.list_profiles(first.id).unwrap(), vec![updated]);
    }
}
