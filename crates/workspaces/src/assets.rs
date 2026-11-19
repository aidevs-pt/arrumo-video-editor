use std::{
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use rusqlite::{Row, TransactionBehavior, params};
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

impl WorkspaceStore {
    pub fn list_assets(&self, workspace_id: i64) -> rusqlite::Result<Vec<WorkspaceAsset>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, kind, byte_size, notes FROM workspace_assets
                 WHERE workspace_id = ?1 ORDER BY id",
            )?
            .query_map([workspace_id], read_asset)?
            .collect()
    }

    pub fn set_asset_notes(
        &self,
        workspace_id: i64,
        asset_id: i64,
        notes: &str,
    ) -> rusqlite::Result<WorkspaceAsset> {
        self.0.query_row(
            "UPDATE workspace_assets SET notes = ?3 WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, name, kind, byte_size, notes",
            params![workspace_id, asset_id, notes],
            read_asset,
        )
    }

    pub fn import_asset(
        &mut self,
        media_root: &Path,
        workspace_id: i64,
        source: &Path,
    ) -> Result<WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid asset filename"))?;
        let kind = media_kind(source)?;
        if !source.metadata()?.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Asset must be a file").into());
        }
        let mut input = File::open(source)?;
        let length = i64::try_from(input.metadata()?.len())?;
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let asset = transaction.query_row(
            "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size)
             VALUES (?1, ?2, ?3, ?4) RETURNING id, workspace_id, name, kind, byte_size, notes",
            params![workspace_id, name, kind, length],
            read_asset,
        )?;
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
