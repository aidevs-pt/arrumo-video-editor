use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

use rusqlite::{Transaction, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::json;

use crate::{
    MotionKind, MotionLibrarySettings, ProductionStage, WorkspaceStore, asset_path, asset_root,
    assets::copy_asset_in,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionArtifact {
    pub kind: MotionKind,
    pub title: String,
    pub usage_markdown: String,
    pub profile_id: Option<i64>,
    pub preview: PathBuf,
    pub thumbnail: Option<PathBuf>,
    pub sound: Option<PathBuf>,
}

impl WorkspaceStore {
    pub fn submit_motion_artifacts(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        artifacts: &[MotionArtifact],
    ) -> Result<i64, Box<dyn Error + Send + Sync>> {
        if artifacts.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "At least one motion artifact is required",
            )
            .into());
        }
        let root = asset_root(&database.canonicalize()?);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        tx.query_row(
            "SELECT id FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3 AND state = 'running'
               AND lease_until > unixepoch()
               AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'assets'",
            params![workspace_id, run_id, attempt],
            |_| Ok(()),
        )?;
        let mut copied = Vec::new();
        let result = (|| -> Result<i64, Box<dyn Error + Send + Sync>> {
            let mut import = |source: &Path| -> Result<i64, Box<dyn Error + Send + Sync>> {
                if !source.is_absolute() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Motion artifact paths must be absolute",
                    )
                    .into());
                }
                let asset = copy_asset_in(&tx, &root, workspace_id, source, false, None)?;
                copied.push(asset_path(&root, workspace_id, asset.id));
                Ok(asset.id)
            };
            let mut items = Vec::new();
            for artifact in artifacts {
                items.push(MotionLibrarySettings {
                    kind: artifact.kind,
                    title: artifact.title.clone(),
                    usage_markdown: artifact.usage_markdown.clone(),
                    profile_id: artifact.profile_id,
                    preview_asset_id: import(&artifact.preview)?,
                    thumbnail_asset_id: artifact
                        .thumbnail
                        .as_deref()
                        .map(&mut import)
                        .transpose()?,
                    sound_asset_id: artifact.sound.as_deref().map(&mut import).transpose()?,
                });
            }
            Ok(self.record_production_result(
                &tx,
                workspace_id,
                run_id,
                attempt,
                ProductionStage::Assets,
                &json!({"motion_items": items}),
            )?)
        })();
        match result {
            Ok(id) => {
                // A failed commit can have an uncertain outcome, so leave the files for recovery.
                tx.commit()?;
                Ok(id)
            }
            Err(error) => {
                for file in copied {
                    let _ = fs::remove_file(file);
                }
                Err(error)
            }
        }
    }
}
