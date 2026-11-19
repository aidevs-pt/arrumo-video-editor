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
        let database = database.canonicalize()?;
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
            let deleted = transaction.execute(
                "DELETE FROM generation_runs
                 WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'
                   AND COALESCE(display_title, json_extract(snapshot_json, '$.draft.title')) = ?3
                   AND NOT EXISTS (SELECT 1 FROM generation_runs WHERE replay_of_run_id = ?2)",
                params![workspace_id, run_id, confirmed_title],
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
        let database = database.canonicalize()?;
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
        Ok(metadata) if metadata.is_dir() && directory.canonicalize()? == directory => Ok(true),
        Ok(_) => Err(io::Error::other("Invalid managed delivery directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
