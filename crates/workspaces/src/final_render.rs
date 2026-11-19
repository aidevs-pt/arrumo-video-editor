use std::{
    error::Error,
    fs, io,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

use crate::format_deliveries::read_settings;
use crate::{
    GenerationPlan, ProductionStage, WorkspaceStore, asset_path, asset_root, delivery_directory,
};

#[derive(Deserialize)]
struct RenderInputs {
    draft: DraftInputs,
    formats: Vec<RenderOutput>,
}

#[derive(Deserialize)]
struct DraftInputs {
    source_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct RenderOutput {
    format: Dimensions,
}

#[derive(Deserialize)]
struct Dimensions {
    id: i64,
    width: u32,
    height: u32,
}

struct ActiveRender<'a> {
    store: &'a WorkspaceStore,
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
}

impl WorkspaceStore {
    pub fn render_primary_videos(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
    ) -> Result<i64, Box<dyn Error>> {
        let (snapshot, plan, index): (String, GenerationPlan, i64) = self.0.query_row(
            "SELECT snapshot_json, plan_json, stage_index FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch() AND plan_json IS NOT NULL",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, read_settings(row.get(1)?, 1)?, row.get(2)?)),
        )?;
        if plan.steps.get(index as usize).map(|step| step.stage)
            != Some(ProductionStage::FinalRender)
        {
            return Err(io::Error::other("The run is not at the final render stage").into());
        }
        let inputs: RenderInputs = serde_json::from_str(&snapshot)?;
        if inputs.draft.source_ids.is_empty() || inputs.formats.is_empty() {
            return Err(io::Error::other(
                "Final clip assembly requires managed source videos and output formats",
            )
            .into());
        }

        let delivery = delivery_directory(database, workspace_id, run_id);
        fs::create_dir_all(&delivery)?;
        let delivery = delivery.canonicalize()?;
        let scratch = delivery.join(format!("render-attempt-{attempt}"));
        fs::create_dir(&scratch)?;
        let renderer = ActiveRender {
            store: self,
            workspace_id,
            run_id,
            attempt,
        };
        let rendered = (|| -> Result<i64, Box<dyn Error>> {
            let root = asset_root(database);
            let sources = inputs
                .draft
                .source_ids
                .iter()
                .map(|id| {
                    let path = asset_path(&root, workspace_id, *id);
                    if !path.symlink_metadata()?.is_file() {
                        return Err(io::Error::other("A managed source video is missing"));
                    }
                    path.canonicalize()
                })
                .collect::<io::Result<Vec<_>>>()?;
            let audio = sources
                .iter()
                .map(|source| renderer.has_audio(source))
                .collect::<Result<Vec<_>, _>>()?;
            let mut filenames = Vec::new();
            for output in inputs.formats {
                let format = output.format;
                renderer.render_format(&scratch, &sources, &audio, &format)?;
                filenames.push(format!("{}.mp4", format.id));
            }
            self.submit_production_result_with(
                workspace_id,
                run_id,
                attempt,
                ProductionStage::FinalRender,
                &json!({ "primary_videos": filenames }),
                || {
                    for filename in &filenames {
                        fs::rename(scratch.join(filename), delivery.join(filename)).map_err(
                            |error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)),
                        )?;
                    }
                    Ok(())
                },
            )
            .map_err(Into::into)
        })();
        if let Err(error) = fs::remove_dir_all(&scratch) {
            eprintln!("Temporary render files could not be cleaned up: {error}");
        }
        rendered
    }
}

impl ActiveRender<'_> {
    fn confirm_lease(&self) -> rusqlite::Result<()> {
        self.store.0.query_row(
            "SELECT id FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch()",
            params![self.workspace_id, self.run_id, self.attempt],
            |_| Ok(()),
        )
    }

    fn run(&self, command: &mut Command, operation: &str) -> Result<(), Box<dyn Error>> {
        self.confirm_lease()?;
        let mut process = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut heartbeat = Instant::now();
        loop {
            if let Some(status) = process.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!("FFmpeg failed while {operation}")).into())
                };
            }
            let lease = if heartbeat.elapsed() >= Duration::from_secs(10) {
                heartbeat = Instant::now();
                self.store
                    .renew_generation_lease(self.workspace_id, self.run_id, self.attempt)
                    .map(|_| ())
            } else {
                self.confirm_lease()
            };
            if let Err(error) = lease {
                let _ = process.kill();
                let _ = process.wait();
                return Err(error.into());
            }
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn has_audio(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        self.confirm_lease()?;
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=index",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("Could not inspect a source video").into());
        }
        Ok(!output.stdout.is_empty())
    }

    fn render_format(
        &self,
        scratch: &Path,
        sources: &[std::path::PathBuf],
        audio: &[bool],
        format: &Dimensions,
    ) -> Result<(), Box<dyn Error>> {
        let Dimensions { id, width, height } = format;
        let pixel_format = if width % 2 == 0 && height % 2 == 0 {
            "yuv420p"
        } else {
            "yuv444p"
        };
        let filter = format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30"
        );
        let mut manifest = String::new();
        for (index, (source, has_audio)) in sources.iter().zip(audio).enumerate() {
            let filename = format!("clip-{id}-{index}.mp4");
            let mut command = Command::new("ffmpeg");
            command
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-filter_threads",
                    "1",
                    "-protocol_whitelist",
                    "file",
                    "-i",
                ])
                .arg(source);
            let audio_source = if *has_audio {
                "0:a:0"
            } else {
                command.args(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]);
                "1:a:0"
            };
            command
                .args([
                    "-map",
                    "0:v:0",
                    "-map",
                    audio_source,
                    "-vf",
                    &filter,
                    "-af",
                    "apad",
                    "-shortest",
                    "-c:v",
                    "libx264",
                    "-threads",
                    "2",
                    "-pix_fmt",
                    pixel_format,
                    "-c:a",
                    "aac",
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                    "-y",
                ])
                .arg(scratch.join(&filename));
            self.run(&mut command, "normalizing a source clip")?;
            manifest.push_str(&format!("file {filename}\n"));
        }
        let list = scratch.join(format!("{id}.ffconcat"));
        fs::write(&list, manifest)?;
        let mut command = Command::new("ffmpeg");
        command
            .args([
                "-nostdin",
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-f",
                "concat",
                "-safe",
                "1",
                "-i",
            ])
            .arg(list)
            .args(["-c", "copy", "-movflags", "+faststart", "-y"])
            .arg(scratch.join(format!("{id}.mp4")));
        self.run(&mut command, "joining source clips")
    }
}
