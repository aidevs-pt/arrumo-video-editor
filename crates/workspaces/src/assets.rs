use std::{
    collections::HashSet,
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use rusqlite::{Row, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WorkspaceAsset {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub kind: String,
    pub byte_size: i64,
    pub notes: String,
}

#[derive(Debug)]
pub struct ImagePreview {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

impl WorkspaceStore {
    pub fn list_assets(&self, workspace_id: i64) -> rusqlite::Result<Vec<WorkspaceAsset>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, kind, byte_size, notes FROM workspace_assets
                 WHERE workspace_id = ?1 AND superseded = 0 ORDER BY id",
            )?
            .query_map([workspace_id], read_asset)?
            .collect()
    }

    pub fn managed_asset_name(
        &self,
        workspace_id: i64,
        asset_id: i64,
        kind: &str,
    ) -> rusqlite::Result<String> {
        self.0.query_row(
            "SELECT name FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND kind = ?3",
            params![workspace_id, asset_id, kind],
            |row| row.get(0),
        )
    }

    pub fn image_preview(
        &self,
        media_root: &Path,
        workspace_id: i64,
        asset_id: i64,
    ) -> Result<Option<ImagePreview>, Box<dyn Error>> {
        const MAX_BYTES: u64 = 4 * 1024 * 1024;
        let size: i64 = self.0.query_row(
            "SELECT byte_size FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND kind = 'image'",
            params![workspace_id, asset_id],
            |row| row.get(0),
        )?;
        if size > MAX_BYTES as i64 {
            return Ok(None);
        }
        let path = asset_path(media_root, workspace_id, asset_id);
        if !path.symlink_metadata()?.is_file() || path.canonicalize()? != path {
            return Err(io::Error::other("Image preview source is unavailable").into());
        }
        let mut bytes = Vec::new();
        File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Ok(None);
        }
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp"
        } else {
            return Ok(None);
        };
        Ok(Some(ImagePreview { mime, bytes }))
    }

    pub fn set_asset_notes(
        &self,
        workspace_id: i64,
        asset_id: i64,
        notes: &str,
    ) -> rusqlite::Result<WorkspaceAsset> {
        self.0.query_row(
            "UPDATE workspace_assets SET notes = ?3 WHERE workspace_id = ?1 AND id = ?2 AND superseded = 0
             RETURNING id, workspace_id, name, kind, byte_size, notes",
            params![workspace_id, asset_id, notes],
            read_asset,
        )
    }

    pub fn set_batch_asset_notes(
        &self,
        workspace_id: i64,
        asset_ids: &[i64],
        notes: &str,
    ) -> rusqlite::Result<Vec<WorkspaceAsset>> {
        let distinct: HashSet<_> = asset_ids.iter().collect();
        if distinct.is_empty() || distinct.len() != asset_ids.len() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let assets = asset_ids
            .iter()
            .map(|&asset_id| self.set_asset_notes(workspace_id, asset_id, notes))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(assets)
    }

    pub fn import_asset(
        &mut self,
        media_root: &Path,
        workspace_id: i64,
        source: &Path,
    ) -> Result<WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        self.copy_asset(media_root, workspace_id, source, false, None)
    }

    pub fn import_thumbnail_reference(
        &mut self,
        media_root: &Path,
        workspace_id: i64,
        source: &Path,
    ) -> Result<WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        self.copy_asset(media_root, workspace_id, source, true, None)
    }

    pub fn replace_asset(
        &mut self,
        media_root: &Path,
        workspace_id: i64,
        asset_id: i64,
        source: &Path,
    ) -> Result<WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        self.copy_asset(media_root, workspace_id, source, false, Some(asset_id))
    }

    fn copy_asset(
        &mut self,
        media_root: &Path,
        workspace_id: i64,
        source: &Path,
        thumbnail_reference: bool,
        replacing: Option<i64>,
    ) -> Result<WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid asset filename"))?;
        let kind = media_kind(source)?;
        let notes = if thumbnail_reference {
            if kind != "image" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Reference must be an image",
                )
                .into());
            }
            "Thumbnail reference"
        } else {
            ""
        };
        if !source.metadata()?.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Asset must be a file").into());
        }
        let mut input = File::open(source)?;
        let length = i64::try_from(input.metadata()?.len())?;
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous_notes: String;
        let notes = if let Some(asset_id) = replacing {
            previous_notes = transaction.query_row(
                "SELECT notes FROM workspace_assets
                 WHERE workspace_id = ?1 AND id = ?2 AND kind = ?3 AND superseded = 0",
                params![workspace_id, asset_id, kind],
                |row| row.get(0),
            )?;
            &previous_notes
        } else {
            notes
        };
        let asset = transaction.query_row(
            "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size, notes)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id, workspace_id, name, kind, byte_size, notes",
            params![workspace_id, name, kind, length, notes],
            read_asset,
        )?;
        if let Some(previous) = replacing {
            replace_live_references(&transaction, workspace_id, previous, asset.id)?;
        }
        let directory = media_root.join(workspace_id.to_string());
        fs::create_dir_all(&directory)?;
        let target = asset_path(media_root, workspace_id, asset.id);
        // This ID has no committed row. An interrupted import may have left its file here.
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let mut output = File::create_new(&target)?;
        let copied = (|| -> io::Result<()> {
            if io::copy(&mut input, &mut output)? != length as u64 {
                return Err(io::Error::other("Asset size changed during import"));
            }
            output.sync_all()?;
            #[cfg(unix)]
            File::open(&directory)?.sync_all()?;
            Ok(())
        })();
        drop(output);
        if let Err(error) = copied {
            let _ = fs::remove_file(&target);
            return Err(error.into());
        }
        // A commit error can have an uncertain outcome, so leave the file for recovery.
        transaction.commit()?;
        Ok(asset)
    }
}

fn replace_live_references(
    tx: &Transaction<'_>,
    workspace_id: i64,
    previous: i64,
    replacement: i64,
) -> rusqlite::Result<()> {
    let ids = params![workspace_id, previous, replacement];
    for statement in [
        "UPDATE video_draft_sources SET asset_id = ?3 WHERE workspace_id = ?1 AND asset_id = ?2",
        "UPDATE workspace_brand_assets SET asset_id = ?3 WHERE workspace_id = ?1 AND asset_id = ?2",
        "UPDATE asset_organization SET asset_id = ?3 WHERE workspace_id = ?1 AND asset_id = ?2",
        "UPDATE motion_library SET preview_asset_id = ?3 WHERE workspace_id = ?1 AND preview_asset_id = ?2",
        "UPDATE motion_library SET thumbnail_asset_id = ?3 WHERE workspace_id = ?1 AND thumbnail_asset_id = ?2",
        "UPDATE motion_library SET sound_asset_id = ?3 WHERE workspace_id = ?1 AND sound_asset_id = ?2",
    ] {
        tx.execute(statement, ids)?;
    }
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
                    "UPDATE {table} SET settings_json = json_set(settings_json, ?4, (
                       SELECT json_group_array(CASE WHEN value = ?2 THEN ?3 ELSE value END ORDER BY key)
                       FROM json_each(settings_json, ?4)
                     ))
                     WHERE {scope} AND EXISTS (SELECT 1 FROM json_each(settings_json, ?4) WHERE value = ?2)"
                ),
                params![workspace_id, previous, replacement, path],
            )?;
        }
    }
    tx.execute(
        "UPDATE workspace_assets SET superseded = 1 WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id, previous],
    )?;
    Ok(())
}

pub fn asset_root(database: &Path) -> PathBuf {
    let mut root = database.as_os_str().to_owned();
    root.push(".media");
    PathBuf::from(root)
}

pub fn asset_path(media_root: &Path, workspace_id: i64, asset_id: i64) -> PathBuf {
    media_root
        .join(workspace_id.to_string())
        .join(asset_id.to_string())
}

fn media_kind(path: &Path) -> io::Result<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "avif" => Ok("image"),
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" => Ok("audio"),
        "mp4" | "mov" | "webm" | "mkv" | "avi" => Ok("video"),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported media extension",
        )),
    }
}

fn read_asset(row: &Row<'_>) -> rusqlite::Result<WorkspaceAsset> {
    Ok(WorkspaceAsset {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        byte_size: row.get(4)?,
        notes: row.get(5)?,
    })
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    use crate::{BrandAssetRole, DraftSettings, RunOptions, SupplementalOutputs};

    #[test]
    fn replacement_retargets_live_references_and_retains_captured_media() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let root = asset_root(&database);
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let original = temp.path().join("original.png");
        let edited = temp.path().join("edited.png");
        let video = temp.path().join("wrong.mp4");
        for (path, bytes) in [
            (&original, "original"),
            (&edited, "edited"),
            (&video, "video"),
        ] {
            fs::write(path, bytes).unwrap();
        }
        let old = store.import_asset(&root, workspace, &original).unwrap();
        let unchanged = store.import_asset(&root, workspace, &original).unwrap();
        store
            .set_asset_notes(workspace, old.id, "Brand reference")
            .unwrap();
        store
            .attach_brand_asset(workspace, old.id, BrandAssetRole::Identity)
            .unwrap();
        let format = store.list_output_formats(workspace).unwrap()[0].id;
        store
            .set_format_delivery(
                workspace,
                format,
                &SupplementalOutputs {
                    thumbnail_reference_ids: vec![unchanged.id, old.id],
                    ..Default::default()
                },
            )
            .unwrap();

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
            .enqueue_video_draft(
                workspace,
                draft.id,
                "before-replacement",
                &RunOptions::default(),
            )
            .unwrap();
        assert!(store.replace_asset(&root, other, old.id, &edited).is_err());
        assert!(
            store
                .replace_asset(&root, workspace, old.id, &video)
                .is_err()
        );
        let new = store
            .replace_asset(&root, workspace, old.id, &edited)
            .unwrap();
        assert_eq!(new.notes, "Brand reference");
        let ids: Vec<_> = store
            .list_assets(workspace)
            .unwrap()
            .iter()
            .map(|asset| asset.id)
            .collect();
        assert_eq!(ids, vec![unchanged.id, new.id]);
        assert_eq!(
            store.list_brand_assets(workspace).unwrap()[0].asset_id,
            new.id
        );
        assert_eq!(
            store
                .format_delivery(workspace, format)
                .unwrap()
                .supplemental
                .thumbnail_reference_ids,
            vec![unchanged.id, new.id]
        );
        assert_eq!(
            fs::read(asset_path(&root, workspace, old.id)).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(asset_path(&root, workspace, new.id)).unwrap(),
            b"edited"
        );
        assert!(
            store
                .replace_asset(&root, workspace, old.id, &edited)
                .is_err()
        );
        assert_eq!(store.recover_asset_cleanup(&database).unwrap(), 0);
        assert!(asset_path(&root, workspace, old.id).is_file());
        assert!(
            store
                .delete_generation(&database, workspace, run.id, &draft.settings.title)
                .unwrap()
        );
        assert_eq!(store.recover_asset_cleanup(&database).unwrap(), 0);
        assert!(!asset_path(&root, workspace, old.id).exists());
        assert!(asset_path(&root, workspace, new.id).is_file());
        assert_eq!(fs::read(original).unwrap(), b"original");
    }
}
