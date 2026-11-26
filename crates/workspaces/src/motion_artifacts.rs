use crate::IoStep;
use crate::Launch;
use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
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
    pub fn claim_queued_motion_stage(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Option<i64>> {
        self.0
            .query_row(
                "UPDATE generation_runs SET state = 'running', attempt = attempt + 1,
                 lease_until = unixepoch() + ?3
             WHERE workspace_id = ?1 AND id = ?2
               AND (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
               AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'assets'
             RETURNING attempt",
                params![
                    workspace_id,
                    run_id,
                    crate::generation_worker::LEASE_SECONDS
                ],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn publish_queued_motion_artifacts(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        artifacts: &[MotionArtifact],
    ) -> Result<Option<i64>, Box<dyn Error + Send + Sync>> {
        let attempt = self.claim_queued_motion_stage(workspace_id, run_id)?;
        let Some(attempt) = attempt else {
            return Ok(None);
        };
        let timing = crate::telemetry::time_step("motion", &[]);
        let outcome =
            self.submit_motion_artifacts(database, workspace_id, run_id, attempt, artifacts);
        if outcome.is_err() {
            // Inside the step's timing, so the warning shows in its trace.
            crate::telemetry::warn("A generation step failed", &[("stage", "motion".into())]);
        }
        timing.finish(outcome.is_ok());
        match outcome {
            Ok(id) => Ok(Some(id)),
            Err(error) => {
                match self.fail_generation_with_error(
                    workspace_id,
                    run_id,
                    attempt,
                    "Motion artifact import failed. Check the files and retry.",
                    &*error,
                ) {
                    // A run stopped or taken over meanwhile is not a failure to report.
                    Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {
                        Err(crate::Handled::new(&*error).into())
                    }
                    Err(failure) => Err(failure.into()),
                }
            }
        }
    }

    pub fn submit_motion_artifacts(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        artifacts: &[MotionArtifact],
    ) -> Result<i64, Box<dyn Error + Send + Sync>> {
        self.submit_motion_artifacts_with(
            database,
            workspace_id,
            run_id,
            attempt,
            artifacts,
            &json!({}),
        )
    }

    /// Like `submit_motion_artifacts`, keeping the other fields of `result` (such as the asset
    /// plan and the animation specs) next to the imported motion items.
    pub(crate) fn submit_motion_artifacts_with(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        artifacts: &[MotionArtifact],
        result: &serde_json::Value,
    ) -> Result<i64, Box<dyn Error + Send + Sync>> {
        if artifacts.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "At least one motion artifact is required",
            )
            .into());
        }
        let root = asset_root(&database.canonicalize().io_step("Resolving a path")?);
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
            let mut import =
                |source: &Path, kind: &str| -> Result<i64, Box<dyn Error + Send + Sync>> {
                    if !source.is_absolute() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Motion artifact paths must be absolute",
                        )
                        .into());
                    }
                    let asset = copy_asset_in(&tx, &root, workspace_id, source, false, None)?;
                    crate::assets::mark_generated(&tx, workspace_id, &[asset.id])?;
                    let path = asset_path(&root, workspace_id, asset.id);
                    copied.push(path.clone());
                    if asset.kind != kind {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Unexpected motion media kind",
                        )
                        .into());
                    }
                    validate_motion_file(&path, &asset.name, kind)?;
                    Ok(asset.id)
                };
            let mut items = Vec::new();
            for artifact in artifacts {
                items.push(MotionLibrarySettings {
                    kind: artifact.kind,
                    title: artifact.title.clone(),
                    usage_markdown: artifact.usage_markdown.clone(),
                    profile_id: artifact.profile_id,
                    preview_asset_id: import(&artifact.preview, "video")?,
                    thumbnail_asset_id: artifact
                        .thumbnail
                        .as_deref()
                        .map(|path| import(path, "image"))
                        .transpose()?,
                    sound_asset_id: artifact
                        .sound
                        .as_deref()
                        .map(|path| import(path, "audio"))
                        .transpose()?,
                });
            }
            let mut result = result.clone();
            result["motion_items"] = json!(items);
            Ok(self.record_production_result(
                &tx,
                workspace_id,
                run_id,
                attempt,
                ProductionStage::Assets,
                &result,
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

impl WorkspaceStore {
    /// Imports a rendered animation and saves it to the library with the spec it came from.
    pub(crate) fn import_motion_library_item(
        &self,
        database: &Path,
        workspace_id: i64,
        artifact: &MotionArtifact,
        spec: &crate::motion_spec::MotionSpec,
    ) -> Result<crate::MotionLibraryItem, Box<dyn Error + Send + Sync>> {
        let root = asset_root(&database.canonicalize().io_step("Resolving a path")?);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let mut copied = Vec::new();
        let result = (|| -> Result<crate::MotionLibraryItem, Box<dyn Error + Send + Sync>> {
            let mut import =
                |source: &Path, kind: &str| -> Result<i64, Box<dyn Error + Send + Sync>> {
                    let asset = copy_asset_in(&tx, &root, workspace_id, source, false, None)?;
                    crate::assets::mark_generated(&tx, workspace_id, &[asset.id])?;
                    let path = asset_path(&root, workspace_id, asset.id);
                    copied.push(path.clone());
                    validate_motion_file(&path, &asset.name, kind)?;
                    Ok(asset.id)
                };
            let settings = MotionLibrarySettings {
                kind: artifact.kind,
                title: artifact.title.clone(),
                usage_markdown: artifact.usage_markdown.clone(),
                profile_id: artifact.profile_id,
                preview_asset_id: import(&artifact.preview, "video")?,
                thumbnail_asset_id: artifact
                    .thumbnail
                    .as_deref()
                    .map(|path| import(path, "image"))
                    .transpose()?,
                sound_asset_id: None,
            };
            let item = self.create_motion_library_item(workspace_id, &settings)?;
            tx.execute(
                "UPDATE motion_library SET spec_json = ?2 WHERE id = ?1",
                params![item.id, serde_json::to_string(spec)?],
            )?;
            Ok(item)
        })();
        match result {
            Ok(item) => {
                tx.commit()?;
                Ok(item)
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

pub(crate) fn validate_motion_file(path: &Path, name: &str, kind: &str) -> io::Result<()> {
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let demuxer = match extension.as_str() {
        "mp4" | "mov" | "m4a" | "avif" => "mov",
        "webm" | "mkv" => "matroska",
        "avi" => "avi",
        "png" => "png_pipe",
        "jpg" | "jpeg" => "jpeg_pipe",
        "webp" => "webp_pipe",
        "gif" => "gif",
        "mp3" => "mp3",
        "wav" => "wav",
        "flac" => "flac",
        "aac" => "aac",
        "ogg" => "ogg",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported motion media format",
            ));
        }
    };
    let selector = if kind == "audio" { "a:0" } else { "V:0" };
    let mut probe = crate::quiet_command(crate::media_tools::ffprobe());
    probe
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-f",
            demuxer,
            "-select_streams",
            selector,
            "-show_entries",
            "stream=codec_type",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            "-i",
        ])
        .arg(path);
    let output = run_probe(&mut probe, Duration::from_secs(10))?;
    let expected = if kind == "audio" {
        b"audio".as_slice()
    } else {
        b"video".as_slice()
    };
    if !output.status.success()
        || !output.stderr.is_empty()
        || output.stdout.trim_ascii() != expected
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Motion media could not be probed",
        ));
    }
    Ok(())
}

fn run_probe(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .launch()?;
    let start = Instant::now();
    let failure = loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output(),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                break io::Error::new(io::ErrorKind::TimedOut, "Motion media probe timed out");
            }
            Err(error) => break error,
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    Err(failure)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn stops_an_unresponsive_motion_probe() {
        let error = run_probe(
            crate::quiet_command("sleep").arg("10"),
            Duration::from_millis(20),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
