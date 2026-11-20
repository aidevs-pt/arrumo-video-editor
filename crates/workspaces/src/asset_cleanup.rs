use std::{error::Error, fs, io, path::Path};

use rusqlite::{Transaction, TransactionBehavior, params};

use crate::{WorkspaceStore, asset_path, asset_root};

impl WorkspaceStore {
    pub fn remove_workspace_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        confirmed_name: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let root = asset_root(&database.canonicalize()?);
        let source = asset_path(&root, workspace_id, asset_id);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let pending: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM pending_asset_cleanup
                            WHERE workspace_id = ?1 AND asset_id = ?2 AND confirmed_name = ?3)",
            params![workspace_id, asset_id, confirmed_name],
            |row| row.get(0),
        )?;
        if !pending {
            let deleted = tx.execute(
                "WITH captured_settings(document) AS (
                    SELECT snapshot_json FROM generation_runs WHERE workspace_id = ?1
                    UNION ALL
                    SELECT delivery.settings_json FROM format_deliveries AS delivery
                        JOIN output_formats AS format ON format.id = delivery.format_id
                        WHERE format.workspace_id = ?1
                    UNION ALL
                    SELECT settings_json FROM profile_deliveries WHERE workspace_id = ?1
                 )
                 DELETE FROM workspace_assets
                 WHERE workspace_id = ?1 AND id = ?2 AND name = ?3
                   AND NOT EXISTS (SELECT 1 FROM workspace_brand_assets WHERE workspace_id = ?1 AND asset_id = ?2)
                   AND NOT EXISTS (SELECT 1 FROM video_draft_sources WHERE workspace_id = ?1 AND asset_id = ?2)
                   AND NOT EXISTS (
                       SELECT 1 FROM production_results AS result
                       JOIN generation_runs AS run ON run.id = result.run_id
                       JOIN json_each(result.result_json, '$.motion_items') AS item
                       WHERE run.workspace_id = ?1 AND item.type = 'object'
                         AND ?2 IN (json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.preview_asset_id'),
                                    json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.thumbnail_asset_id'),
                                    json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.sound_asset_id'))
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM production_results AS result
                       JOIN generation_runs AS run ON run.id = result.run_id
                       JOIN json_each(result.result_json, '$.supplemental_media') AS item
                       WHERE run.workspace_id = ?1 AND item.type = 'object'
                         AND json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.asset_id') = ?2
                   )
                   AND NOT EXISTS (SELECT 1 FROM completed_media WHERE asset_id = ?2)
                   AND NOT EXISTS (SELECT 1 FROM motion_library WHERE workspace_id = ?1
                                   AND ?2 IN (preview_asset_id, thumbnail_asset_id, sound_asset_id))
                   AND NOT EXISTS (
                       SELECT 1 FROM captured_settings, json_tree(document) AS reference
                       WHERE reference.type = 'integer' AND reference.atom = ?2
                         AND (replace(reference.path, char(34), '') = '$.draft.source_ids'
                           OR replace(reference.path, char(34), '') LIKE '%.thumbnail_reference_ids'
                           OR (replace(reference.path, char(34), '') LIKE '$.brand_assets[%]' AND reference.key = 'asset_id'))
                   )",
                params![workspace_id, asset_id, confirmed_name],
            )?;
            if deleted == 0 {
                return Ok(false);
            }
            check_asset_file(&source)?;
            tx.execute(
                "INSERT INTO pending_asset_cleanup (workspace_id, asset_id, confirmed_name) VALUES (?1, ?2, ?3)",
                params![workspace_id, asset_id, confirmed_name],
            )?;
        }
        tx.commit()?;

        self.finish_asset_cleanup(&root, workspace_id, asset_id)?;
        Ok(true)
    }

    pub fn recover_asset_cleanup(&self, database: &Path) -> Result<usize, Box<dyn Error>> {
        let root = asset_root(&database.canonicalize()?);
        let pending: Vec<(i64, i64)> = self
            .0
            .prepare("SELECT workspace_id, asset_id FROM pending_asset_cleanup")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut remaining = 0;
        for (workspace_id, asset_id) in pending {
            if self
                .finish_asset_cleanup(&root, workspace_id, asset_id)
                .is_err()
            {
                remaining += 1;
            }
        }
        let superseded: Vec<(i64, i64, String)> = self
            .0
            .prepare("SELECT workspace_id, id, name FROM workspace_assets WHERE superseded = 1")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (workspace_id, asset_id, name) in superseded {
            // Use the same reference checks and durable cleanup record as explicit removal.
            if self
                .remove_workspace_asset(database, workspace_id, asset_id, &name)
                .is_err()
            {
                remaining += 1;
            }
        }
        Ok(remaining)
    }

    fn finish_asset_cleanup(
        &self,
        root: &Path,
        workspace_id: i64,
        asset_id: i64,
    ) -> Result<(), Box<dyn Error>> {
        let live: bool = self.0.query_row(
            "SELECT EXISTS (SELECT 1 FROM workspace_assets WHERE id = ?1)",
            [asset_id],
            |row| row.get(0),
        )?;
        if live {
            return Err(io::Error::other("Cannot clean a live asset").into());
        }
        let source = asset_path(root, workspace_id, asset_id);
        if check_asset_file(&source)? {
            match fs::remove_file(source) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.0.execute(
            "DELETE FROM pending_asset_cleanup WHERE workspace_id = ?1 AND asset_id = ?2",
            params![workspace_id, asset_id],
        )?;
        Ok(())
    }
}

fn check_asset_file(source: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.is_file() && source.canonicalize()? == source => Ok(true),
        Ok(_) => Err(io::Error::other("Invalid managed asset file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunOptions, SupplementalOutputs};

    #[test]
    fn recovery_cleans_deleted_assets_and_preserves_live_assets() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let original = temp.path().join("reference.png");
        fs::write(&original, b"fixture").unwrap();
        let root = asset_root(&database);
        let deleted = store.import_asset(&root, workspace, &original).unwrap();
        let live = store.import_asset(&root, workspace, &original).unwrap();

        // Recreate the database state committed before filesystem cleanup.
        store
            .0
            .execute("DELETE FROM workspace_assets WHERE id = ?1", [deleted.id])
            .unwrap();
        for asset in [&deleted, &live] {
            store
                .0
                .execute(
                    "INSERT INTO pending_asset_cleanup VALUES (?1, ?2, ?3)",
                    params![workspace, asset.id, asset.name],
                )
                .unwrap();
        }
        drop(store);

        let store = WorkspaceStore::open(&database).unwrap();
        assert_eq!(store.recover_asset_cleanup(&database).unwrap(), 1);
        assert!(!asset_path(&root, workspace, deleted.id).exists());
        assert_eq!(
            fs::read(asset_path(&root, workspace, live.id)).unwrap(),
            b"fixture"
        );
        assert_eq!(fs::read(original).unwrap(), b"fixture");
        assert_eq!(store.recover_asset_cleanup(&database).unwrap(), 1);
    }

    #[test]
    fn captured_thumbnail_references_survive_live_settings_changes() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let source = temp.path().join("reference.png");
        fs::write(&source, b"fixture").unwrap();
        let asset = store
            .import_asset(&asset_root(&database), workspace, &source)
            .unwrap();
        let format = store.list_output_formats(workspace).unwrap()[0].id;
        store
            .set_format_delivery(
                workspace,
                format,
                &SupplementalOutputs {
                    thumbnail_reference_ids: vec![asset.id],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            !store
                .remove_workspace_asset(&database, workspace, asset.id, &asset.name)
                .unwrap()
        );
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Fixture".into(),
                    intent: "Test references".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "snapshot", &RunOptions::default())
            .unwrap();
        store
            .set_format_delivery(workspace, format, &SupplementalOutputs::default())
            .unwrap();
        assert!(
            !store
                .remove_workspace_asset(&database, workspace, asset.id, &asset.name)
                .unwrap()
        );
        assert!(
            store
                .delete_generation(&database, workspace, run.id, &draft.settings.title)
                .unwrap()
        );
        assert!(
            store
                .remove_workspace_asset(&database, workspace, asset.id, &asset.name)
                .unwrap()
        );
        assert!(source.exists());
        assert!(!asset_path(&asset_root(&database), workspace, asset.id).exists());
    }
}
