use crate::IoStep;
use std::{error::Error, fs, io, path::Path};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::{WorkspaceStore, asset_path, asset_root};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AssetRemoval {
    Removed,
    /// No asset with that ID and name, or it was already removed.
    Missing,
    InUse {
        uses: Vec<AssetUse>,
    },
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AssetUse {
    pub kind: AssetUseKind,
    pub name: String,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum AssetUseKind {
    /// A video that is queued, running, waiting for review or can be retried.
    OpenVideo,
    FinishedVideo,
    Animation,
}

/// Runs that can still pick their work up again read their captured inputs.
const OPEN_RUN_STATES: &str = "'queued', 'running', 'paused', 'failed'";

fn asset_uses(
    tx: &Transaction,
    workspace_id: i64,
    asset_id: i64,
) -> rusqlite::Result<Vec<AssetUse>> {
    let title = "COALESCE(run.display_title, json_extract(run.snapshot_json, '$.draft.title'), '')";
    let queries = [
        (
            AssetUseKind::OpenVideo,
            format!(
                "SELECT DISTINCT {title} FROM generation_runs AS run, json_tree(run.snapshot_json) AS reference
                 WHERE run.workspace_id = ?1 AND run.state IN ({OPEN_RUN_STATES})
                   AND reference.type = 'integer' AND reference.atom = ?2
                   AND (replace(reference.path, char(34), '') = '$.draft.source_ids'
                     OR replace(reference.path, char(34), '') LIKE '%.thumbnail_reference_ids'
                     OR (replace(reference.path, char(34), '') LIKE '$.brand_assets[%]' AND reference.key = 'asset_id'))"
            ),
        ),
        (
            AssetUseKind::FinishedVideo,
            format!(
                "SELECT DISTINCT {title} FROM completed_media AS media
                   JOIN generation_runs AS run ON run.id = media.run_id
                 WHERE run.workspace_id = ?1 AND media.asset_id = ?2
                 UNION
                 SELECT DISTINCT {title} FROM production_results AS result
                   JOIN generation_runs AS run ON run.id = result.run_id
                   JOIN json_each(result.result_json, '$.motion_items') AS item
                 WHERE run.workspace_id = ?1 AND item.type = 'object'
                   AND ?2 IN (json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.preview_asset_id'),
                              json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.thumbnail_asset_id'),
                              json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.sound_asset_id'))
                 UNION
                 SELECT DISTINCT {title} FROM production_results AS result
                   JOIN generation_runs AS run ON run.id = result.run_id
                   JOIN json_each(result.result_json, '$.supplemental_media') AS item
                 WHERE run.workspace_id = ?1 AND item.type = 'object'
                   AND json_extract(CASE WHEN item.type = 'object' THEN item.value END, '$.asset_id') = ?2"
            ),
        ),
        (
            AssetUseKind::Animation,
            "SELECT title FROM motion_library WHERE workspace_id = ?1
               AND (?2 IN (preview_asset_id, thumbnail_asset_id, sound_asset_id)
                 OR EXISTS (SELECT 1 FROM json_tree(CASE WHEN json_valid(spec_json) THEN spec_json END) AS layer
                            WHERE layer.key IN ('assetId', 'soundAssetId') AND layer.atom = ?2))
             UNION
             SELECT item.title FROM motion_library_versions AS version
               JOIN motion_library AS item ON item.workspace_id = version.workspace_id AND item.id = version.item_id
             WHERE version.workspace_id = ?1
               AND (?2 IN (version.preview_asset_id, version.thumbnail_asset_id, version.sound_asset_id)
                 OR EXISTS (SELECT 1 FROM json_tree(CASE WHEN json_valid(version.spec_json) THEN version.spec_json END) AS layer
                            WHERE layer.key IN ('assetId', 'soundAssetId') AND layer.atom = ?2))"
                .to_owned(),
        ),
    ];
    let mut uses = Vec::new();
    for (kind, query) in queries {
        let names = tx
            .prepare(&query)?
            .query_map(params![workspace_id, asset_id], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        uses.extend(names.into_iter().map(|name| AssetUse { kind, name }));
    }
    Ok(uses)
}

/// Settings the creator edits let go of a removed asset. Brand roles, profile shares,
/// organization and transcripts follow through their foreign keys.
fn release_live_references(
    tx: &Transaction,
    workspace_id: i64,
    asset_id: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM video_draft_sources WHERE workspace_id = ?1 AND asset_id = ?2",
        params![workspace_id, asset_id],
    )?;
    for (table, scope) in [
        (
            "format_deliveries",
            "format_id IN (SELECT id FROM output_formats WHERE workspace_id = ?1)",
        ),
        ("profile_deliveries", "workspace_id = ?1"),
    ] {
        for path in [
            "$.thumbnail_reference_ids",
            "$.portrait_derivatives.source.thumbnail_reference_ids",
        ] {
            tx.execute(
                &format!(
                    // json() keeps the array an array, as in asset replacement.
                    "UPDATE {table} SET settings_json = json_set(settings_json, ?3, json((
                       SELECT json_group_array(value ORDER BY key)
                       FROM json_each(settings_json, ?3) WHERE value IS NOT ?2
                     )))
                     WHERE {scope} AND EXISTS (SELECT 1 FROM json_each(settings_json, ?3) WHERE value = ?2)"
                ),
                params![workspace_id, asset_id, path],
            )?;
        }
    }
    Ok(())
}

impl WorkspaceStore {
    pub fn remove_workspace_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        confirmed_name: &str,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(
            self.remove_asset_explained(database, workspace_id, asset_id, confirmed_name)?
                == AssetRemoval::Removed,
        )
    }

    /// Removes an asset the creator asked to delete. Settings that point at it (brand roles,
    /// thumbnail references, draft sources, profile shares, organization) let go of it; what
    /// it is still needed for, a video that can still continue, a finished video or an
    /// animation, keeps it and is named in the answer.
    pub fn remove_asset_explained(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        confirmed_name: &str,
    ) -> Result<AssetRemoval, Box<dyn Error>> {
        let root = asset_root(&database.canonicalize().io_step("Resolving a path")?);
        let source = asset_path(&root, workspace_id, asset_id);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let pending: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM pending_asset_cleanup
                            WHERE workspace_id = ?1 AND asset_id = ?2 AND confirmed_name = ?3)",
            params![workspace_id, asset_id, confirmed_name],
            |row| row.get(0),
        )?;
        if !pending {
            let exists: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND name = ?3)",
                params![workspace_id, asset_id, confirmed_name],
                |row| row.get(0),
            )?;
            if !exists {
                return Ok(AssetRemoval::Missing);
            }
            let uses = asset_uses(&tx, workspace_id, asset_id)?;
            if !uses.is_empty() {
                return Ok(AssetRemoval::InUse { uses });
            }
            release_live_references(&tx, workspace_id, asset_id)?;
            tx.execute(
                "DELETE FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND name = ?3",
                params![workspace_id, asset_id, confirmed_name],
            )?;
            check_asset_file(&source)?;
            tx.execute(
                "INSERT INTO pending_asset_cleanup (workspace_id, asset_id, confirmed_name) VALUES (?1, ?2, ?3)",
                params![workspace_id, asset_id, confirmed_name],
            )?;
        }
        tx.commit()?;

        self.finish_asset_cleanup(&root, workspace_id, asset_id)?;
        Ok(AssetRemoval::Removed)
    }

    /// Removes several assets, each confirmed by its own name, and answers for each one.
    pub fn remove_assets_explained(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_ids: &[i64],
    ) -> Result<Vec<(i64, AssetRemoval)>, Box<dyn Error>> {
        let mut results = Vec::with_capacity(asset_ids.len());
        for &asset_id in asset_ids {
            let name: Option<String> = self
                .0
                .query_row(
                    "SELECT name FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2",
                    params![workspace_id, asset_id],
                    |row| row.get(0),
                )
                .optional()?;
            let result = match name {
                Some(name) => {
                    self.remove_asset_explained(database, workspace_id, asset_id, &name)?
                }
                None => AssetRemoval::Missing,
            };
            results.push((asset_id, result));
        }
        Ok(results)
    }

    pub fn recover_asset_cleanup(&self, database: &Path) -> Result<usize, Box<dyn Error>> {
        let root = asset_root(&database.canonicalize().io_step("Resolving a path")?);
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
        remove_cached_frames(root, workspace_id, asset_id)?;
        self.0.execute(
            "DELETE FROM pending_asset_cleanup WHERE workspace_id = ?1 AND asset_id = ?2",
            params![workspace_id, asset_id],
        )?;
        Ok(())
    }
}

/// Stills taken of a removed video live beside the media folder (see [`crate::frame_cache`]).
fn remove_cached_frames(media_root: &Path, workspace_id: i64, asset_id: i64) -> io::Result<()> {
    let Some(database) = media_root
        .to_str()
        .and_then(|root| root.strip_suffix(".media"))
    else {
        return Ok(());
    };
    let directory = crate::frame_cache(Path::new(database)).join(workspace_id.to_string());
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let prefix = format!("{asset_id}-");
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(&prefix))
            && entry.file_type()?.is_file()
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn check_asset_file(source: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.is_file() && crate::assets::is_canonical(source)? => Ok(true),
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

    fn studio_with_reference() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        WorkspaceStore,
        i64,
        crate::WorkspaceAsset,
        i64,
    ) {
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
        (temp, database, store, workspace, asset, format)
    }

    #[test]
    fn removal_releases_the_settings_that_point_at_the_asset() {
        let (temp, database, store, workspace, asset, format) = studio_with_reference();
        store
            .attach_brand_asset(workspace, asset.id, crate::BrandAssetRole::Identity)
            .unwrap();
        assert_eq!(
            store
                .remove_asset_explained(&database, workspace, asset.id, &asset.name)
                .unwrap(),
            AssetRemoval::Removed
        );
        assert!(
            store
                .format_delivery(workspace, format)
                .unwrap()
                .supplemental
                .thumbnail_reference_ids
                .is_empty()
        );
        assert!(store.list_brand_assets(workspace).unwrap().is_empty());
        assert!(!asset_path(&asset_root(&database), workspace, asset.id).exists());
        assert!(temp.path().join("reference.png").exists());
    }

    #[test]
    fn a_video_that_can_continue_keeps_its_assets_until_it_is_finished() {
        let (_temp, database, mut store, workspace, asset, _format) = studio_with_reference();
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
        assert_eq!(
            store
                .remove_asset_explained(&database, workspace, asset.id, &asset.name)
                .unwrap(),
            AssetRemoval::InUse {
                uses: vec![AssetUse {
                    kind: AssetUseKind::OpenVideo,
                    name: "Fixture".into()
                }]
            }
        );
        store
            .0
            .execute(
                "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                [run.id],
            )
            .unwrap();
        assert!(
            store
                .remove_workspace_asset(&database, workspace, asset.id, &asset.name)
                .unwrap()
        );
    }

    #[test]
    fn several_assets_are_removed_by_their_own_names() {
        let (temp, database, mut store, workspace, first, _format) = studio_with_reference();
        let other = temp.path().join("other.png");
        fs::write(&other, b"fixture").unwrap();
        let second = store
            .import_asset(&asset_root(&database), workspace, &other)
            .unwrap();
        let results = store
            .remove_assets_explained(&database, workspace, &[first.id, second.id, 999])
            .unwrap();
        assert_eq!(
            results,
            vec![
                (first.id, AssetRemoval::Removed),
                (second.id, AssetRemoval::Removed),
                (999, AssetRemoval::Missing)
            ]
        );
        assert!(store.list_assets(workspace).unwrap().is_empty());
    }
}
