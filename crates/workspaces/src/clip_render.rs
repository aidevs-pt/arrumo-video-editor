use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

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

impl Dimensions {
    fn scale_preview(&mut self) {
        // Keep the output aspect ratio while limiting the longest edge to 480 pixels.
        let longest = self.width.max(self.height);
        if longest > 480 {
            let scale = |edge| (u64::from(edge) * 480 / u64::from(longest)).max(1) as u32;
            self.width = scale(self.width);
            self.height = scale(self.height);
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PreviewCut {
    asset_id: i64,
    start_ms: u32,
    end_ms: u32,
}

pub(crate) fn preview_cuts(
    result: &Value,
    source_ids: &[i64],
) -> io::Result<Option<Vec<PreviewCut>>> {
    let Some(value) = result.get("cuts") else {
        return Ok(None);
    };
    let cuts: Vec<PreviewCut> = serde_json::from_value(value.clone())?;
    if cuts.is_empty()
        || cuts
            .iter()
            .any(|cut| cut.end_ms <= cut.start_ms || !source_ids.contains(&cut.asset_id))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Preview cuts require increasing times and assets from the frozen source list",
        ));
    }
    Ok(Some(cuts))
}

struct RenderClip {
    path: PathBuf,
    cut: Option<PreviewCut>,
    has_audio: bool,
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
        self.render_clips(
            database,
            ActiveRender {
                store: self,
                workspace_id,
                run_id,
                attempt,
            },
            ProductionStage::FinalRender,
            None,
        )
    }

    pub fn render_preview_videos(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        edit: &Value,
    ) -> Result<i64, Box<dyn Error>> {
        if !edit.is_object() {
            return Err(io::Error::other("Preview edit must be an object").into());
        }
        self.render_clips(
            database,
            ActiveRender {
                store: self,
                workspace_id,
                run_id,
                attempt,
            },
            ProductionStage::Preview,
            Some(edit),
        )
    }

    fn render_clips(
        &self,
        database: &Path,
        renderer: ActiveRender<'_>,
        stage: ProductionStage,
        edit: Option<&Value>,
    ) -> Result<i64, Box<dyn Error>> {
        let ActiveRender {
            workspace_id,
            run_id,
            attempt,
            ..
        } = renderer;
        let (snapshot, plan, index): (String, GenerationPlan, i64) = self.0.query_row(
            "SELECT snapshot_json, plan_json, stage_index FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch() AND plan_json IS NOT NULL",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, read_settings(row.get(1)?, 1)?, row.get(2)?)),
        )?;
        if plan.steps.get(index as usize).map(|step| step.stage) != Some(stage) {
            return Err(io::Error::other("The run is not at the requested render stage").into());
        }
        let inputs: RenderInputs = serde_json::from_str(&snapshot)?;
        if inputs.draft.source_ids.is_empty() || inputs.formats.is_empty() {
            return Err(io::Error::other(
                "Clip assembly requires managed source videos and output formats",
            )
            .into());
        }

        let edit = match edit {
            Some(edit) => edit.clone(),
            None => self.accepted_preview(run_id, &plan)?,
        };
        let sources = match preview_cuts(&edit, &inputs.draft.source_ids)? {
            Some(cuts) => cuts
                .into_iter()
                .map(|cut| (cut.asset_id, Some(cut)))
                .collect(),
            None => inputs
                .draft
                .source_ids
                .into_iter()
                .map(|id| (id, None))
                .collect::<Vec<_>>(),
        };

        let delivery = delivery_directory(database, workspace_id, run_id);
        fs::create_dir_all(&delivery)?;
        let delivery = delivery.canonicalize()?;
        let scratch = delivery.join(format!("render-attempt-{attempt}"));
        fs::create_dir(&scratch)?;
        let rendered = (|| -> Result<i64, Box<dyn Error>> {
            let root = asset_root(database);
            let mut clips = Vec::new();
            for (id, cut) in sources {
                let path = asset_path(&root, workspace_id, id);
                if !path.symlink_metadata()?.is_file() {
                    return Err(io::Error::other("A managed source video is missing").into());
                }
                let path = path.canonicalize()?;
                let has_audio = renderer.inspect_source(&path, cut.as_ref())?;
                clips.push(RenderClip {
                    path,
                    cut,
                    has_audio,
                });
            }
            let mut filenames = Vec::new();
            for output in inputs.formats {
                let mut format = output.format;
                if stage == ProductionStage::Preview {
                    format.scale_preview();
                }
                renderer.render_format(&scratch, &clips, &format)?;
                let filename = if stage == ProductionStage::Preview {
                    format!("preview-{attempt}-{}.mp4", format.id)
                } else {
                    format!("{}.mp4", format.id)
                };
                filenames.push((format.id, filename));
            }
            let result = if stage == ProductionStage::Preview {
                let mut result = edit;
                result["preview_videos"] =
                    json!(filenames.iter().map(|(_, name)| name).collect::<Vec<_>>());
                result
            } else {
                json!({ "primary_videos": filenames.iter().map(|(_, name)| name).collect::<Vec<_>>() })
            };
            self.submit_production_result_with(
                workspace_id,
                run_id,
                attempt,
                stage,
                &result,
                || {
                    for (id, filename) in &filenames {
                        fs::rename(scratch.join(format!("{id}.mp4")), delivery.join(filename))
                            .map_err(|error| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                            })?;
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
    fn accepted_preview(
        &self,
        run_id: i64,
        plan: &GenerationPlan,
    ) -> Result<Value, Box<dyn Error>> {
        let (preview_index, preview_step) = plan
            .steps
            .iter()
            .enumerate()
            .find(|(_, step)| step.stage == ProductionStage::Preview)
            .ok_or_else(|| io::Error::other("No preview step exists in the saved plan"))?;
        let preview: String = self.0.query_row(
            "SELECT result_json FROM production_results
             WHERE run_id = ?1 AND stage_index = ?2
               AND ((?3 AND decision = 'approved') OR (NOT ?3 AND decision IS NULL))
             ORDER BY id DESC LIMIT 1",
            params![run_id, preview_index as i64, preview_step.review_required],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&preview)?)
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

    fn inspect_source(
        &self,
        path: &Path,
        cut: Option<&PreviewCut>,
    ) -> Result<bool, Box<dyn Error>> {
        self.confirm_lease()?;
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-show_entries",
                "stream=codec_type,duration:format=duration",
                "-of",
                "json",
            ])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("Could not inspect a source video").into());
        }
        let info: Value = serde_json::from_slice(&output.stdout)?;
        let streams = info["streams"]
            .as_array()
            .ok_or_else(|| io::Error::other("No streams found in managed source"))?;
        let video = streams
            .iter()
            .find(|stream| stream["codec_type"] == "video")
            .ok_or_else(|| io::Error::other("Managed source has no video stream"))?;
        if let Some(cut) = cut {
            let duration = [&video["duration"], &info["format"]["duration"]]
                .into_iter()
                .filter_map(Value::as_str)
                .find_map(|duration| {
                    duration
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                })
                .ok_or_else(|| io::Error::other("Could not determine duration of a cut source"))?;
            if f64::from(cut.end_ms) > (duration * 1000.0).round() {
                return Err(io::Error::other(format!(
                    "Preview cut ends beyond the duration of source {}",
                    cut.asset_id
                ))
                .into());
            }
        }
        Ok(streams.iter().any(|stream| stream["codec_type"] == "audio"))
    }

    fn render_format(
        &self,
        scratch: &Path,
        clips: &[RenderClip],
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
        for (index, clip) in clips.iter().enumerate() {
            let filename = format!("clip-{id}-{index}.mp4");
            let mut command = Command::new("ffmpeg");
            command.args([
                "-nostdin",
                "-v",
                "error",
                "-filter_threads",
                "1",
                "-protocol_whitelist",
                "file",
            ]);
            if let Some(cut) = &clip.cut {
                command.args(["-ss", &format!("{}ms", cut.start_ms)]);
            }
            command.arg("-i").arg(&clip.path);
            let audio_source = if clip.has_audio {
                "0:a:0"
            } else {
                command.args(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"]);
                "1:a:0"
            };
            if let Some(cut) = &clip.cut {
                command.args(["-t", &format!("{}ms", cut.end_ms - cut.start_ms)]);
            }
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
