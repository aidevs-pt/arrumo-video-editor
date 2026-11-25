use crate::IoStep;
use std::{error::Error, fs, io, path::Path};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use serde::Serialize;

use crate::{WorkspaceStore, delivery_directory};

#[derive(Debug, Serialize)]
pub struct DeliveryCleanupReport {
    pub removed_run_ids: Vec<i64>,
    pub pending_run_ids: Vec<i64>,
}

impl WorkspaceStore {
    pub fn delete_completed_generation(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        confirmed_title: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.delete_run_and_delivery(database, workspace_id, run_id, confirmed_title, true)
    }

    pub fn delete_generation(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        confirmed_title: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.delete_run_and_delivery(database, workspace_id, run_id, confirmed_title, false)
    }

    fn delete_run_and_delivery(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        confirmed_title: &str,
        completed_only: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let database = database.canonicalize().io_step("Resolving a path")?;
        let directory = delivery_directory(&database, workspace_id, run_id);
        let transaction = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let already_pending = transaction
            .query_row(
                "SELECT 1 FROM pending_delivery_cleanup
             WHERE workspace_id = ?1 AND run_id = ?2 AND confirmed_title = ?3",
                params![workspace_id, run_id, confirmed_title],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !already_pending {
            // Replays keep their own snapshot, so they outlive the run they replayed.
            transaction.execute(
                "UPDATE generation_runs SET replay_of_run_id = NULL, replay_mode = NULL
                 WHERE workspace_id = ?1 AND replay_of_run_id = ?2",
                params![workspace_id, run_id],
            )?;
            let deleted = transaction.execute(
                "DELETE FROM generation_runs
                 WHERE workspace_id = ?1 AND id = ?2 AND (NOT ?4 OR state = 'completed')
                   AND COALESCE(display_title, json_extract(snapshot_json, '$.draft.title')) = ?3",
                params![workspace_id, run_id, confirmed_title, completed_only],
            )?;
            if deleted == 0 {
                return Ok(false);
            }
            check_delivery_directory(&directory)?;
            transaction.execute(
                "INSERT INTO pending_delivery_cleanup (workspace_id, run_id, confirmed_title)
                 VALUES (?1, ?2, ?3)",
                params![workspace_id, run_id, confirmed_title],
            )?;
        }
        transaction.commit()?;

        self.finish_delivery_cleanup(&database, workspace_id, run_id)?;
        Ok(true)
    }

    pub fn recover_delivery_cleanup(
        &self,
        database: &Path,
        workspace_id: i64,
    ) -> Result<DeliveryCleanupReport, Box<dyn Error>> {
        self.recover_pending_deliveries(database, Some(workspace_id))
    }

    pub fn recover_all_delivery_cleanup(
        &self,
        database: &Path,
    ) -> Result<DeliveryCleanupReport, Box<dyn Error>> {
        self.recover_pending_deliveries(database, None)
    }

    fn recover_pending_deliveries(
        &self,
        database: &Path,
        workspace_id: Option<i64>,
    ) -> Result<DeliveryCleanupReport, Box<dyn Error>> {
        let database = database.canonicalize().io_step("Resolving a path")?;
        let ids: Vec<(i64, i64)> = self
            .0
            .prepare(
                "SELECT workspace_id, run_id FROM pending_delivery_cleanup
                 WHERE ?1 IS NULL OR workspace_id = ?1 ORDER BY workspace_id, run_id",
            )?
            .query_map([workspace_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut report = DeliveryCleanupReport {
            removed_run_ids: Vec::new(),
            pending_run_ids: Vec::new(),
        };
        for (workspace_id, run_id) in ids {
            if self
                .finish_delivery_cleanup(&database, workspace_id, run_id)
                .is_ok()
            {
                report.removed_run_ids.push(run_id);
            } else {
                report.pending_run_ids.push(run_id);
            }
        }
        Ok(report)
    }

    fn finish_delivery_cleanup(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
    ) -> Result<(), Box<dyn Error>> {
        let live_run: bool = self.0.query_row(
            "SELECT EXISTS (SELECT 1 FROM generation_runs WHERE id = ?1)",
            [run_id],
            |row| row.get(0),
        )?;
        if live_run {
            return Err(io::Error::other("Cannot clean a live run").into());
        }
        let directory = delivery_directory(database, workspace_id, run_id);
        if check_delivery_directory(&directory)? {
            match fs::remove_dir_all(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.0.execute(
            "DELETE FROM pending_delivery_cleanup WHERE workspace_id = ?1 AND run_id = ?2",
            params![workspace_id, run_id],
        )?;
        Ok(())
    }
}

fn check_delivery_directory(directory: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() && crate::assets::is_canonical(directory)? => Ok(true),
        Ok(_) => Err(io::Error::other("Invalid managed delivery directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunOptions};
    use serde_json::json;

    #[test]
    fn active_deletion_revokes_the_lease_and_preserves_the_draft() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Run to remove".into(),
                    intent: "An example".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "test", &RunOptions::default())
            .unwrap();
        let claim = store.claim_generation(workspace).unwrap().unwrap();
        let delivery = delivery_directory(&database, workspace, run.id);
        fs::create_dir_all(&delivery).unwrap();
        fs::write(delivery.join("partial.mp4"), b"fixture").unwrap();
        assert!(
            !store
                .delete_completed_generation(&database, workspace, run.id, &draft.settings.title)
                .unwrap()
        );
        assert!(
            !store
                .delete_generation(&database, workspace, run.id, "Wrong title")
                .unwrap()
        );
        assert!(
            store
                .delete_generation(&database, workspace, run.id, &draft.settings.title)
                .unwrap()
        );
        assert!(!delivery.exists());
        assert!(
            store
                .checkpoint_generation(workspace, run.id, claim.attempt, &json!({}))
                .is_err()
        );
        assert!(store.generation_run(workspace, run.id).unwrap().is_none());
        assert_eq!(store.list_video_drafts(workspace).unwrap(), vec![draft]);
    }

    #[test]
    fn deleting_a_replayed_video_keeps_its_replay() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "First cut".into(),
                    intent: "An example".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let first = store
            .enqueue_video_draft(workspace, draft.id, "first", &RunOptions::default())
            .unwrap();
        store
            .0
            .execute(
                "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                [first.id],
            )
            .unwrap();
        let replay = store
            .replay_generation(workspace, first.id, "again", crate::ReplayMode::Current)
            .unwrap();
        assert!(
            !store
                .delete_completed_generation(&database, workspace, first.id, "Wrong title")
                .unwrap()
        );
        let source: Option<i64> = store
            .0
            .query_row(
                "SELECT replay_of_run_id FROM generation_runs WHERE id = ?1",
                [replay.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, Some(first.id), "a refused deletion keeps the link");
        assert!(
            store
                .delete_completed_generation(&database, workspace, first.id, "First cut")
                .unwrap()
        );
        assert!(store.generation_run(workspace, first.id).unwrap().is_none());
        assert!(
            store
                .generation_run(workspace, replay.id)
                .unwrap()
                .is_some()
        );
    }
}
