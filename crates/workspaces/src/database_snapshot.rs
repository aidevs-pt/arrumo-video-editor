use std::{
    error::Error,
    io::{self, Read, Seek},
    path::Path,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::WorkspaceStore;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSnapshot {
    pub sha256: String,
    pub byte_length: u64,
}

impl WorkspaceStore {
    pub fn snapshot_database(
        &self,
        destination: &Path,
    ) -> Result<DatabaseSnapshot, Box<dyn Error>> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        let path = pending
            .path()
            .to_str()
            .ok_or_else(|| io::Error::other("SQLite snapshot path must be UTF-8"))?;
        self.0.execute("VACUUM INTO ?1", [path])?;

        // SQLite leaves syncing the VACUUM INTO output to the caller.
        pending.as_file().sync_all()?;
        let byte_length = pending.as_file().metadata()?.len();
        pending.rewind()?;
        let mut digest = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = pending.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        let sha256 = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        pending.persist_noclobber(destination)?;
        Ok(DatabaseSnapshot {
            sha256,
            byte_length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_committed_database_state_and_refuses_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("workspaces.sqlite");
        let destination = temp.path().join("snapshot.sqlite");
        let store = WorkspaceStore::open(&source).unwrap();
        store.0.pragma_update(None, "journal_mode", "WAL").unwrap();
        let workspace = store.create("Studio").unwrap();
        store
            .set_workspace_context(workspace.id, "Captured context")
            .unwrap();
        let snapshot = store.snapshot_database(&destination).unwrap();
        store
            .set_workspace_context(workspace.id, "Changed context")
            .unwrap();

        let restored = WorkspaceStore::open(&destination).unwrap();
        assert_eq!(
            restored.workspace_context(workspace.id).unwrap().as_deref(),
            Some("Captured context")
        );
        let original_bytes = std::fs::read(&destination).unwrap();
        assert_eq!(snapshot.byte_length, original_bytes.len() as u64);
        assert_eq!(
            snapshot.sha256,
            crate::plugin_package::package_sha256(&original_bytes).unwrap()
        );
        assert!(store.snapshot_database(&destination).is_err());
        assert!(store.snapshot_database(&source).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), original_bytes);
        assert_eq!(
            store.workspace_context(workspace.id).unwrap().as_deref(),
            Some("Changed context")
        );
    }
}
