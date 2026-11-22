use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rusqlite::{Transaction, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::format_deliveries::read_settings;
use crate::motion_spec::{
    MotionSpec, overlay_graph, render_spec, render_spec_with_audio, run_ffmpeg,
};
use crate::{
    GenerationPlan, ProductionStage, WorkspaceStore, asset_path, asset_root, delivery_directory,
};

#[derive(Deserialize)]
struct RenderInputs {
    draft: DraftInputs,
    formats: Vec<RenderOutput>,
    preview_settings: Option<FrozenPreviewSettings>,
}

#[derive(Deserialize)]
struct FrozenPreviewSettings {
    effective_max_edge: u32,
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
    fn scale_preview(&mut self, max_edge: u32) {
        let longest = self.width.max(self.height);
        if longest > max_edge {
            // Even sizes: FFmpeg rounds subsampled video to even rows and columns when scaling.
            let scale = |edge| {
                ((u64::from(edge) * u64::from(max_edge) / u64::from(longest)).max(2) & !1) as u32
            };
            self.width = scale(self.width);
            self.height = scale(self.height);
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
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

pub(crate) fn validate_preview_edit(
    result: &Value,
    source_ids: &[i64],
    format_ids: &[i64],
) -> io::Result<()> {
    preview_cuts(result, source_ids)?;
    if let Some(overrides) = result.get("format_cuts") {
        let overrides = overrides.as_object().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Format cuts must be an object keyed by frozen format ID",
            )
        })?;
        for (key, value) in overrides {
            if !format_ids.iter().any(|id| id.to_string() == *key) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Preview cut format is not in the frozen run",
                ));
            }
            if !value.is_null() {
                preview_cuts(&json!({ "cuts": value }), source_ids)?;
            }
        }
    }
    Ok(())
}

fn cuts_for_format(
    edit: &Value,
    format_id: i64,
    source_ids: &[i64],
) -> io::Result<Option<Vec<PreviewCut>>> {
    match edit
        .get("format_cuts")
        .and_then(|cuts| cuts.get(format_id.to_string()))
    {
        Some(Value::Null) => Ok(None),
        Some(value) => preview_cuts(&json!({ "cuts": value }), source_ids),
        None => preview_cuts(edit, source_ids),
    }
}

/// A full-frame animation played before a cut; `before_cut` equal to the number of cuts plays
/// it at the end.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, PartialEq, Eq)]
pub(crate) struct MotionInsert {
    pub before_cut: usize,
    pub animation: usize,
}

/// An animation drawn over a cut, starting `at_ms` into it.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, PartialEq, Eq)]
pub(crate) struct MotionOverlay {
    pub cut: usize,
    pub animation: usize,
    pub at_ms: u32,
}

/// The approved animations an edit of recorded footage places, and where.
#[derive(Default)]
pub(crate) struct EditMotion {
    animations: Vec<MotionSpec>,
    inserts: Vec<MotionInsert>,
    overlays: Vec<MotionOverlay>,
}

const MAX_PLACEMENTS: usize = 40;

impl EditMotion {
    fn inserts_before(&self, cut: usize) -> impl Iterator<Item = &MotionSpec> {
        self.inserts
            .iter()
            .filter(move |insert| insert.before_cut == cut)
            .filter_map(|insert| self.animations.get(insert.animation))
    }

    fn overlays_on(&self, cut: usize) -> impl Iterator<Item = (&MotionSpec, f64)> {
        self.overlays
            .iter()
            .filter(move |overlay| overlay.cut == cut)
            .filter_map(|overlay| {
                self.animations
                    .get(overlay.animation)
                    .map(|spec| (spec, f64::from(overlay.at_ms) / 1000.0))
            })
    }
}

/// A scene's voiced narration: the managed audio file and the words it says.
type NarrationTrack = (Option<PathBuf>, Option<String>);

fn narration_tracks(
    database: &Path,
    workspace_id: i64,
    run_id: i64,
    edit: &Value,
    scenes: usize,
) -> Result<Vec<NarrationTrack>, Box<dyn Error>> {
    let Some(files) = edit.get("narration_audio").and_then(Value::as_array) else {
        return Ok(vec![(None, None); scenes]);
    };
    (0..scenes)
        .map(|index| {
            let Some(name) = files.get(index).and_then(Value::as_str) else {
                return Ok((None, None));
            };
            if !crate::narration::narration_file_name(name) {
                return Err(
                    io::Error::other("A narration file name is not one Arrumo wrote").into(),
                );
            }
            let path = crate::delivery_verification::managed_delivery_file(
                database,
                workspace_id,
                run_id,
                name,
            )?;
            let words = edit["narration"][index].as_str().map(str::to_owned);
            Ok((Some(path), words))
        })
        .collect()
}

/// Delivered loudness: -14 LUFS integrated with -1.5 dBTP peaks, where social platforms
/// normalize playback.
const MASTER_LOUDNESS: &str = "I=-14:TP=-1.5:LRA=11";
/// Audio quieter than this is room tone or near silence; raising it to the target would mostly
/// amplify noise, so it is left as it is.
const QUIETEST_MASTERED_LUFS: f64 = -45.0;

/// The second loudnorm pass from the first pass's JSON report, or `None` when the audio is
/// silent, nearly silent or could not be measured.
fn loudness_pass(report: &str) -> Option<String> {
    let start = report.rfind('{')?;
    let end = start + report[start..].find('}')?;
    let json: Value = serde_json::from_str(&report[start..=end]).ok()?;
    let field = |name: &str| {
        json.get(name)?
            .as_str()?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    };
    let input = field("input_i").filter(|loudness| *loudness > QUIETEST_MASTERED_LUFS)?;
    Some(format!(
        "loudnorm={MASTER_LOUDNESS}:measured_I={input}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true,aresample=48000",
        field("input_tp")?,
        field("input_lra")?,
        field("input_thresh")?,
        field("target_offset")?,
    ))
}

/// What the host measured on one rendered format, kept with the stage result.
#[derive(Debug, PartialEq)]
pub(crate) struct RenderCheck {
    pub seconds: f64,
    pub expected_seconds: f64,
}

impl RenderCheck {
    fn to_json(&self) -> Value {
        let round = |value: f64| (value * 1000.0).round() / 1000.0;
        json!({
            "seconds": round(self.seconds),
            "expected_seconds": round(self.expected_seconds),
        })
    }
}

/// Every concatenated part may gain or lose an audio frame, so allow a little per part.
fn duration_matches(seconds: f64, parts: &[f64]) -> bool {
    let expected: f64 = parts.iter().sum();
    (seconds - expected).abs() <= 0.1 + 0.05 * parts.len() as f64
}

/// Fades the first and last 30 ms of a cut's audio so joined cuts don't click.
fn cut_audio_filter(length_ms: u32) -> String {
    let fade = (length_ms / 4).min(30);
    if fade == 0 {
        return "apad".to_owned();
    }
    let out = f64::from(length_ms - fade) / 1000.0;
    let fade = f64::from(fade) / 1000.0;
    format!("afade=t=in:st=0:d={fade},afade=t=out:st={out}:d={fade},apad")
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
    pub fn review_preview_file(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        format_id: i64,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let result: String = self.0.query_row(
            "SELECT result.result_json FROM generation_runs AS run
             JOIN production_results AS result ON result.run_id = run.id AND result.id = run.pending_review_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3
               AND run.state = 'paused' AND result.decision IS NULL AND result.stage_index = run.stage_index
               AND json_extract(run.plan_json, '$.steps[' || run.stage_index || '].stage') = 'preview'",
            params![workspace_id, run_id, result_id],
            |row| row.get(0),
        )?;
        let result: Value = serde_json::from_str(&result)?;
        let mut files = crate::preview_review::preview_files(&result)?;
        let filename = files
            .remove(&format_id)
            .ok_or_else(|| io::Error::other("Preview file is not in the pending review"))?;
        Ok(crate::delivery_verification::managed_delivery_file(
            database,
            workspace_id,
            run_id,
            &filename,
        )?)
    }

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
        let edit = match edit {
            Some(edit) => edit.clone(),
            None => self.accepted_preview(run_id, &plan)?,
        };
        let scenes = self.storyboard_scenes(workspace_id, &edit)?;
        let narration = match &scenes {
            Some(scenes) => narration_tracks(database, workspace_id, run_id, &edit, scenes.len())?,
            None => Vec::new(),
        };
        if (inputs.draft.source_ids.is_empty() && scenes.is_none()) || inputs.formats.is_empty() {
            return Err(io::Error::other(
                "Rendering requires managed source videos or a storyboard, and output formats",
            )
            .into());
        }
        let format_ids = inputs
            .formats
            .iter()
            .map(|entry| entry.format.id)
            .collect::<Vec<_>>();
        if scenes.is_none() {
            validate_preview_edit(&edit, &inputs.draft.source_ids, &format_ids)?;
        }
        let motion = self.edit_motion(workspace_id, &edit)?;
        let music = self.edit_music(&asset_root(database), workspace_id, &edit)?;

        let revision = if stage == ProductionStage::Preview {
            crate::preview_review::preview_revision(&self.0, run_id, index)?
        } else {
            None
        };

        if let Some(revision) = &revision {
            for filename in revision.approved.values() {
                crate::delivery_verification::managed_delivery_file(
                    database,
                    workspace_id,
                    run_id,
                    filename,
                )?;
            }
        }

        let preview_max_edge = inputs.preview_settings.map_or(
            crate::preview_settings::LEGACY_PREVIEW_MAX_EDGE,
            |settings| settings.effective_max_edge,
        );
        let (delivery, scratch) = {
            // Serialize directory creation with deletion and recheck the lease before touching disk.
            let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
            tx.query_row(
                "SELECT 1 FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
                 AND state = 'running' AND lease_until > unixepoch()",
                params![workspace_id, run_id, attempt],
                |_| Ok(()),
            )?;
            let delivery = delivery_directory(database, workspace_id, run_id);
            fs::create_dir_all(&delivery)?;
            let delivery = delivery.canonicalize()?;
            let scratch = delivery.join(format!("render-attempt-{attempt}"));
            fs::create_dir(&scratch)?;
            tx.commit()?;
            (delivery, scratch)
        };
        let rendered = (|| -> Result<i64, Box<dyn Error>> {
            let root = asset_root(database);
            let mut clip_sets = Vec::new();
            let mut filenames = Vec::new();
            let mut checks = serde_json::Map::new();
            for output in inputs.formats {
                let mut format = output.format;
                if revision
                    .as_ref()
                    .is_some_and(|revision| !revision.unresolved.contains_key(&format.id))
                {
                    continue;
                }
                if stage == ProductionStage::Preview {
                    format.scale_preview(preview_max_edge);
                }
                if let Some(scenes) = &scenes {
                    let check =
                        renderer.render_scenes(&root, &scratch, scenes, &narration, &format)?;
                    checks.insert(format.id.to_string(), check.to_json());
                    if stage == ProductionStage::FinalRender {
                        // Storyboards have no speech to transcribe; caption their on-screen text.
                        fs::write(
                            scratch.join(format!("{}.srt", format.id)),
                            crate::motion_spec::storyboard_subtitles(
                                scenes,
                                &narration
                                    .iter()
                                    .map(|(_, words)| words.as_deref())
                                    .collect::<Vec<_>>(),
                            ),
                        )?;
                    }
                    let filename = if stage == ProductionStage::Preview {
                        format!("preview-{attempt}-{}.mp4", format.id)
                    } else {
                        format!("{}.mp4", format.id)
                    };
                    filenames.push((format.id, filename));
                    continue;
                }
                let cuts = cuts_for_format(&edit, format.id, &inputs.draft.source_ids)?;
                let clips_index = if let Some(index) =
                    clip_sets.iter().position(|(cached, _)| cached == &cuts)
                {
                    index
                } else {
                    let clips =
                        renderer.inspect_clips(&root, &inputs.draft.source_ids, cuts.as_deref())?;
                    clip_sets.push((cuts, clips));
                    clip_sets.len() - 1
                };
                let check = renderer.render_format(
                    &root,
                    &scratch,
                    &clip_sets[clips_index].1,
                    &format,
                    &motion,
                    music.as_deref(),
                )?;
                checks.insert(format.id.to_string(), check.to_json());
                let filename = if stage == ProductionStage::Preview {
                    format!("preview-{attempt}-{}.mp4", format.id)
                } else {
                    format!("{}.mp4", format.id)
                };
                filenames.push((format.id, filename));
            }
            let mut result = if stage == ProductionStage::Preview {
                let mut result = edit;
                result["preview_videos"] =
                    json!(filenames.iter().map(|(_, name)| name).collect::<Vec<_>>());
                result
            } else {
                json!({ "primary_videos": filenames.iter().map(|(_, name)| name).collect::<Vec<_>>() })
            };
            if checks.is_empty() {
                if let Some(result) = result.as_object_mut() {
                    result.remove("render_checks");
                }
            } else {
                result["render_checks"] = Value::Object(checks);
            }
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
                        let captions = scratch.join(format!("{id}.srt"));
                        let destination = delivery.join(format!("{id}.srt"));
                        if captions.is_file() && destination.symlink_metadata().is_err() {
                            fs::rename(captions, destination).map_err(|error| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                            })?;
                        }
                    }
                    Ok(())
                },
            )
            .map_err(Into::into)
        })();
        if let Err(error) = fs::remove_dir_all(&scratch)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("Temporary render files could not be cleaned up: {error}");
        }
        rendered
    }
    /// Renders the run's accepted preview edit at any size into `<directory>/0.mp4`, for
    /// derivatives such as portrait cuts. The caller holds the run's current lease.
    pub(crate) fn render_accepted_edit(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        (width, height): (u32, u32),
        directory: &Path,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let (snapshot, plan): (String, GenerationPlan) = self.0.query_row(
            "SELECT snapshot_json, plan_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch() AND plan_json IS NOT NULL",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, read_settings(row.get(1)?, 1)?)),
        )?;
        let inputs: RenderInputs = serde_json::from_str(&snapshot)?;
        let edit = self.accepted_preview(run_id, &plan)?;
        let renderer = ActiveRender {
            store: self,
            workspace_id,
            run_id,
            attempt,
        };
        let root = asset_root(database);
        let format = Dimensions {
            id: 0,
            width,
            height,
        };
        if let Some(scenes) = self.storyboard_scenes(workspace_id, &edit)? {
            let narration = narration_tracks(database, workspace_id, run_id, &edit, scenes.len())?;
            renderer.render_scenes(&root, directory, &scenes, &narration, &format)?;
        } else {
            let source_ids = &inputs.draft.source_ids;
            let cuts = preview_cuts(&edit, source_ids)?;
            let clips = renderer.inspect_clips(&root, source_ids, cuts.as_deref())?;
            let motion = self.edit_motion(workspace_id, &edit)?;
            let music = self.edit_music(&root, workspace_id, &edit)?;
            renderer.render_format(&root, directory, &clips, &format, &motion, music.as_deref())?;
        }
        Ok(directory.join("0.mp4"))
    }

    /// The animations an edit of recorded footage places, validated against the workspace.
    fn edit_motion(&self, workspace_id: i64, edit: &Value) -> Result<EditMotion, Box<dyn Error>> {
        let Some(animations) = edit.get("animations").and_then(Value::as_array) else {
            return Ok(EditMotion::default());
        };
        let images: Vec<i64> = self
            .workspace_images(workspace_id)?
            .into_iter()
            .map(|image| image.id)
            .collect();
        let animations = animations
            .iter()
            .take(crate::motion_design::MAX_RUN_ANIMATIONS * 4)
            .map(|spec| MotionSpec::from_answer(spec, |id| images.contains(&id)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        let list = |key: &str| edit.get(key).cloned().unwrap_or(Value::Array(Vec::new()));
        let inserts: Vec<MotionInsert> = serde_json::from_value(list("motion_inserts"))?;
        let overlays: Vec<MotionOverlay> = serde_json::from_value(list("motion_overlays"))?;
        if inserts.len() > MAX_PLACEMENTS
            || overlays.len() > MAX_PLACEMENTS
            || inserts
                .iter()
                .any(|insert| insert.animation >= animations.len())
            || overlays
                .iter()
                .any(|overlay| overlay.animation >= animations.len())
        {
            return Err(io::Error::other("The edit places animations it does not include").into());
        }
        Ok(EditMotion {
            animations,
            inserts,
            overlays,
        })
    }

    /// The workspace audio file an edit lays under the video as background music, if any.
    fn edit_music(
        &self,
        root: &Path,
        workspace_id: i64,
        edit: &Value,
    ) -> Result<Option<PathBuf>, Box<dyn Error>> {
        let Some(music) = edit.get("music") else {
            return Ok(None);
        };
        let invalid =
            || io::Error::other("The edit's music is not an audio file in this workspace");
        let id = music["asset_id"].as_i64().ok_or_else(invalid)?;
        self.0
            .query_row(
                "SELECT 1 FROM workspace_assets
                 WHERE workspace_id = ?1 AND id = ?2 AND kind = 'audio' AND superseded = 0",
                params![workspace_id, id],
                |_| Ok(()),
            )
            .map_err(|_| invalid())?;
        Ok(Some(asset_path(root, workspace_id, id).canonicalize()?))
    }

    /// The validated scenes of a storyboard edit, or `None` for an edit of recorded footage.
    fn storyboard_scenes(
        &self,
        workspace_id: i64,
        edit: &Value,
    ) -> Result<Option<Vec<MotionSpec>>, Box<dyn Error>> {
        let Some(scenes) = edit.get("scenes") else {
            return Ok(None);
        };
        let scenes = scenes
            .as_array()
            .filter(|scenes| !scenes.is_empty() && scenes.len() <= crate::preview_edit::MAX_SCENES)
            .ok_or_else(|| io::Error::other("A storyboard needs between one and forty scenes"))?;
        let images: Vec<i64> = self
            .workspace_images(workspace_id)?
            .into_iter()
            .map(|image| image.id)
            .collect();
        scenes
            .iter()
            .map(|scene| {
                MotionSpec::from_answer(scene, |id| images.contains(&id))
                    .map_err(|reason| io::Error::other(reason).into())
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()
            .map(Some)
    }

    pub(crate) fn accepted_preview(
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
    /// Renders each scene at the format's size and joins them into `<format id>.mp4`.
    fn render_scenes(
        &self,
        root: &Path,
        scratch: &Path,
        scenes: &[MotionSpec],
        narration: &[NarrationTrack],
        format: &Dimensions,
    ) -> Result<RenderCheck, Box<dyn Error>> {
        let directory = scratch.join(format!("scenes-{}", format.id));
        fs::create_dir(&directory)?;
        let images = |id: i64| asset_path(root, self.workspace_id, id).canonicalize().ok();
        let mut heartbeat = Instant::now();
        let mut check = || -> Result<(), Box<dyn Error>> {
            if heartbeat.elapsed() >= Duration::from_secs(10) {
                heartbeat = Instant::now();
                self.store
                    .renew_generation_lease(self.workspace_id, self.run_id, self.attempt)?;
            } else {
                self.confirm_lease()?;
            }
            Ok(())
        };
        let mut manifest = String::new();
        let mut parts = Vec::with_capacity(scenes.len());
        for (index, scene) in scenes.iter().enumerate() {
            let name = format!("scene-{index}.mp4");
            render_spec_with_audio(
                scene,
                (format.width, format.height),
                &directory,
                &name,
                &images,
                narration.get(index).and_then(|(audio, _)| audio.as_deref()),
                &mut check,
            )?;
            parts.push(probe_duration(&directory.join(&name))?);
            manifest.push_str(&format!("file {name}\n"));
        }
        fs::write(directory.join("scenes.ffconcat"), manifest)?;
        let output = scratch.join(format!("{}-joined.mp4", format.id));
        let args: Vec<String> = [
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
            "scenes.ffconcat",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            "-y",
        ]
        .map(String::from)
        .into_iter()
        .chain([output.to_string_lossy().into_owned()])
        .collect();
        run_ffmpeg(&directory, &args, &mut check)?;
        let finished = scratch.join(format!("{}.mp4", format.id));
        self.master_audio(&output, &finished)?;
        self.check_render(&finished, &parts)
    }

    fn inspect_clips(
        &self,
        root: &Path,
        source_ids: &[i64],
        cuts: Option<&[PreviewCut]>,
    ) -> Result<Vec<RenderClip>, Box<dyn Error>> {
        let sources: Vec<_> = match cuts {
            Some(cuts) => cuts.iter().map(|cut| (cut.asset_id, Some(cut))).collect(),
            None => source_ids.iter().map(|&id| (id, None)).collect(),
        };
        sources
            .into_iter()
            .map(|(id, cut)| {
                let path = asset_path(root, self.workspace_id, id);
                if !path.symlink_metadata()?.is_file() {
                    return Err(io::Error::other("A managed source video is missing").into());
                }
                let path = path.canonicalize()?;
                let has_audio = self.inspect_source(&path, cut)?;
                Ok(RenderClip {
                    path,
                    cut: cut.copied(),
                    has_audio,
                })
            })
            .collect()
    }

    fn confirm_lease(&self) -> rusqlite::Result<()> {
        self.store.0.query_row(
            "SELECT id FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch()",
            params![self.workspace_id, self.run_id, self.attempt],
            |_| Ok(()),
        )
    }

    fn run(&self, command: &mut Command, operation: &str) -> Result<(), Box<dyn Error>> {
        self.run_output(command, operation).map(drop)
    }

    /// Runs FFmpeg while keeping the lease, returning what it wrote to stderr.
    fn run_output(&self, command: &mut Command, operation: &str) -> Result<String, Box<dyn Error>> {
        self.confirm_lease()?;
        let mut process = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        // FFmpeg runs with `-v error`, so its error output stays small.
        let errors = process.stderr.take().map(|mut stderr| {
            thread::spawn(move || {
                let mut text = String::new();
                let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
                text
            })
        });
        let mut heartbeat = Instant::now();
        loop {
            if let Some(status) = process.try_wait()? {
                let errors = errors
                    .and_then(|reader| reader.join().ok())
                    .unwrap_or_default();
                return if status.success() {
                    Ok(errors)
                } else {
                    let tail: Vec<&str> = errors.lines().rev().take(4).collect();
                    Err(io::Error::other(format!(
                        "FFmpeg failed while {operation}: {}",
                        tail.join(" | ")
                    ))
                    .into())
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

    /// Writes `input` to `output` with its audio at the delivery loudness, measured first so
    /// the gain is linear. A silent or nearly silent video is moved unchanged.
    fn master_audio(&self, input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        let measured = self.run_output(
            Command::new("ffmpeg")
                .args(["-nostdin", "-hide_banner", "-nostats", "-v", "info"])
                .args(["-protocol_whitelist", "file", "-i"])
                .arg(input)
                .args(["-map", "0:a:0", "-af"])
                .arg(format!("loudnorm={MASTER_LOUDNESS}:print_format=json"))
                .args(["-f", "null", "-"]),
            "measuring loudness",
        )?;
        let Some(filter) = loudness_pass(&measured) else {
            fs::rename(input, output)?;
            return Ok(());
        };
        self.run(
            Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file",
                    "-i",
                ])
                .arg(input)
                .args(["-map", "0:v:0", "-map", "0:a:0", "-c:v", "copy", "-af"])
                .arg(filter)
                .args(["-c:a", "aac", "-ar", "48000", "-ac", "2"])
                .args(["-movflags", "+faststart", "-y"])
                .arg(output),
            "evening out the loudness",
        )?;
        fs::remove_file(input)?;
        Ok(())
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
        root: &Path,
        scratch: &Path,
        clips: &[RenderClip],
        format: &Dimensions,
        motion: &EditMotion,
        music: Option<&Path>,
    ) -> Result<RenderCheck, Box<dyn Error>> {
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
        let images = |asset: i64| {
            asset_path(root, self.workspace_id, asset)
                .canonicalize()
                .ok()
        };
        let mut check = || -> Result<(), Box<dyn Error>> { Ok(self.confirm_lease()?) };
        let mut parts = Vec::new();
        let mut insert = |cut: usize,
                          manifest: &mut String,
                          parts: &mut Vec<f64>|
         -> Result<(), Box<dyn Error>> {
            for (number, spec) in motion.inserts_before(cut).enumerate() {
                let name = format!("insert-{id}-{cut}-{number}.mp4");
                render_spec(spec, (*width, *height), scratch, &name, &images, &mut check)?;
                parts.push(probe_duration(&scratch.join(&name))?);
                manifest.push_str(&format!("file {name}\n"));
            }
            Ok(())
        };
        for (index, clip) in clips.iter().enumerate() {
            insert(index, &mut manifest, &mut parts)?;
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
            let mut graph = format!("[0:v]{filter}[base];");
            let mut video = "base".to_owned();
            let mut inputs = Vec::new();
            for (number, (spec, at)) in motion.overlays_on(index).enumerate() {
                let first_input = if clip.has_audio { 1 } else { 2 }
                    + inputs.iter().filter(|arg| *arg == "-i").count();
                video = overlay_graph(
                    spec,
                    (*width, *height),
                    scratch,
                    &format!("o{index}x{number}"),
                    at,
                    &video,
                    first_input,
                    &mut graph,
                    &mut inputs,
                    &images,
                )?;
            }
            graph.push_str(&format!("[{video}]null[video]"));
            command.args(&inputs).current_dir(scratch);
            if let Some(cut) = &clip.cut {
                command.args(["-t", &format!("{}ms", cut.end_ms - cut.start_ms)]);
            }
            let audio_filter = clip.cut.as_ref().map_or_else(
                || "apad".to_owned(),
                |cut| cut_audio_filter(cut.end_ms - cut.start_ms),
            );
            command
                .args([
                    "-map",
                    "[video]",
                    "-map",
                    audio_source,
                    "-filter_complex",
                    &graph,
                    "-af",
                    &audio_filter,
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
            parts.push(probe_duration(&scratch.join(&filename))?);
            manifest.push_str(&format!("file {filename}\n"));
        }
        insert(clips.len(), &mut manifest, &mut parts)?;
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
            .arg(scratch.join(format!("{id}-joined.mp4")));
        self.run(&mut command, "joining source clips")?;
        let output = scratch.join(format!("{id}.mp4"));
        let joined = scratch.join(format!("{id}-joined.mp4"));
        match music {
            Some(music) => {
                let mastered = scratch.join(format!("{id}-mastered.mp4"));
                self.master_audio(&joined, &mastered)?;
                self.lay_music(&mastered, music, &output)?;
                fs::remove_file(mastered)?;
            }
            None => self.master_audio(&joined, &output)?,
        }
        self.check_render(&output, &parts)
    }

    /// Loops `music` under the mastered speech at a background level, lowered about 10 dB while
    /// anyone speaks and faded out over the last 1.5 seconds, then limited to the delivery peak.
    fn lay_music(&self, input: &Path, music: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        let fade_at = (probe_duration(input)? - 1.5).max(0.0);
        let stereo = "aformat=sample_rates=48000:channel_layouts=stereo";
        self.run(
            Command::new("ffmpeg")
                .args(["-nostdin", "-v", "error", "-protocol_whitelist", "file", "-i"])
                .arg(input)
                .args(["-stream_loop", "-1", "-i"])
                .arg(music)
                .arg("-filter_complex")
                .arg(format!(
                    "[1:a:0]{stereo},loudnorm=I=-30:TP=-9:LRA=7[bed];\
                     [0:a:0]{stereo},asplit=2[speech][key];\
                     [bed][key]sidechaincompress=threshold=0.02:ratio=8:attack=20:release=400[ducked];\
                     [speech][ducked]amix=inputs=2:normalize=0:duration=first,\
                     afade=t=out:st={fade_at:.3}:d=1.5,alimiter=limit=0.84:level=false,aresample=48000[audio]"
                ))
                .args(["-map", "0:v:0", "-map", "[audio]", "-c:v", "copy"])
                .args(["-c:a", "aac", "-ar", "48000", "-ac", "2"])
                .args(["-movflags", "+faststart", "-y"])
                .arg(output),
            "laying the music under the video",
        )
    }

    /// Compares the finished video's duration with the parts it was joined from.
    fn check_render(&self, video: &Path, parts: &[f64]) -> Result<RenderCheck, Box<dyn Error>> {
        let seconds = probe_duration(video)?;
        let expected_seconds = parts.iter().sum();
        if !duration_matches(seconds, parts) {
            return Err(io::Error::other(format!(
                "The rendered video lasts {seconds:.2} s, but its parts add up to {expected_seconds:.2} s"
            ))
            .into());
        }
        Ok(RenderCheck {
            seconds,
            expected_seconds,
        })
    }
}

fn probe_duration(path: &Path) -> Result<f64, Box<dyn Error>> {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-protocol_whitelist", "file"])
        .args(["-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(path)
        .output()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|seconds| output.status.success() && seconds.is_finite())
        .ok_or_else(|| io::Error::other("Could not measure a rendered video's duration").into())
}

#[cfg(test)]
mod tests {
    use super::{cut_audio_filter, duration_matches};

    #[test]
    fn cut_audio_fades_in_and_out() {
        assert_eq!(
            cut_audio_filter(2000),
            "afade=t=in:st=0:d=0.03,afade=t=out:st=1.97:d=0.03,apad"
        );
        assert_eq!(
            cut_audio_filter(80),
            "afade=t=in:st=0:d=0.02,afade=t=out:st=0.06:d=0.02,apad"
        );
        assert_eq!(cut_audio_filter(3), "apad");
    }

    #[test]
    fn loudness_uses_the_measurement_and_skips_silence() {
        let loud = "[Parsed_loudnorm_0 @ 0x1]\n{\n\t\"input_i\" : \"-23.10\",\n\t\"input_tp\" : \"-6.02\",\n\t\"input_lra\" : \"3.40\",\n\t\"input_thresh\" : \"-33.30\",\n\t\"target_offset\" : \"0.12\"\n}\nsize=N/A";
        assert_eq!(
            super::loudness_pass(loud).as_deref(),
            Some(
                "loudnorm=I=-14:TP=-1.5:LRA=11:measured_I=-23.1:measured_TP=-6.02:measured_LRA=3.4:measured_thresh=-33.3:offset=0.12:linear=true,aresample=48000"
            )
        );
        let silent = "{\"input_i\" : \"-inf\", \"input_tp\" : \"-inf\", \"input_lra\" : \"0.00\", \"input_thresh\" : \"-70.00\", \"target_offset\" : \"inf\"}";
        assert_eq!(super::loudness_pass(silent), None);
        assert_eq!(super::loudness_pass("no report"), None);
        let room_tone = loud.replace("-23.10", "-52.00");
        assert_eq!(super::loudness_pass(&room_tone), None);
    }

    #[test]
    fn duration_allows_a_frame_per_part() {
        assert!(duration_matches(10.12, &[4.0, 6.0]));
        assert!(!duration_matches(10.5, &[4.0, 6.0]));
        assert!(!duration_matches(8.0, &[4.0, 6.0]));
    }
}
