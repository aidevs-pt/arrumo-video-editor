use crate::{IoStep, Launch, Step};
use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
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
    #[serde(default)]
    grade: Option<Grade>,
}

/// A light colour correction for one cut, applied while its clip is extracted so it costs no
/// extra encode. Neutral values leave the picture unchanged.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, PartialEq, Eq)]
pub(crate) struct Grade {
    /// Hundredths of the full range, -15 to 15.
    pub brightness: i16,
    /// Percent, 80 to 130.
    pub contrast: i16,
    /// Percent, 60 to 140.
    pub saturation: i16,
    /// -10 (cooler) to 10 (warmer).
    pub warmth: i16,
}

impl Grade {
    pub(crate) const NEUTRAL: Self = Self {
        brightness: 0,
        contrast: 100,
        saturation: 100,
        warmth: 0,
    };

    /// The grade within its ranges, or `None` when it would change nothing.
    pub(crate) fn clamped(self) -> Option<Self> {
        let grade = Self {
            brightness: self.brightness.clamp(-15, 15),
            contrast: self.contrast.clamp(80, 130),
            saturation: self.saturation.clamp(60, 140),
            warmth: self.warmth.clamp(-10, 10),
        };
        (grade != Self::NEUTRAL).then_some(grade)
    }

    fn filter(self) -> String {
        let warmth = f64::from(self.warmth) * 0.006;
        format!(
            "eq=brightness={:.2}:contrast={:.2}:saturation={:.2},colorbalance=rm={warmth:.3}:bm={:.3}:rh={:.3}:bh={:.3},",
            f64::from(self.brightness) / 100.0,
            f64::from(self.contrast) / 100.0,
            f64::from(self.saturation) / 100.0,
            -warmth,
            warmth / 2.0,
            -warmth / 2.0,
        )
    }
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
    /// Where one part ends and the next begins, in seconds on the rendered timeline.
    pub joins: Vec<f64>,
}

impl RenderCheck {
    fn to_json(&self) -> Value {
        let round = |value: f64| (value * 1000.0).round() / 1000.0;
        json!({
            "seconds": round(self.seconds),
            "expected_seconds": round(self.expected_seconds),
            "joins": self.joins.iter().map(|&at| round(at)).collect::<Vec<_>>(),
        })
    }
}

/// A sound laid over an edit of recorded footage: a voice-over line or a sound effect, starting
/// `at` seconds into cut `cut`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioCue {
    pub cut: usize,
    pub at: f64,
    pub path: PathBuf,
    pub voice: bool,
    /// The source footage and the moment in it (ms) the cue lands on, so a format cut its own
    /// way, such as a portrait version, plays it at the same moment.
    pub source: Option<(i64, u32)>,
}

/// Where a cue plays in a format's clips: the clip's index and seconds into it. A cue anchored
/// to footage plays wherever that moment survives in this format; a cue whose moment was cut
/// from the format is left out.
fn cue_position(cue: &AudioCue, clips: &[RenderClip]) -> Option<(usize, f64)> {
    let Some((asset, moment)) = cue.source else {
        return Some((cue.cut, cue.at));
    };
    let here = |clip: &RenderClip| {
        let (start, end) = clip
            .cut
            .map_or((0, u32::MAX), |cut| (cut.start_ms, cut.end_ms));
        (clip.asset_id == asset && (start..end).contains(&moment))
            .then(|| f64::from(moment - start) / 1000.0)
    };
    // The cut the agent named, when this format keeps it; footage used twice, such as a
    // teaser, would otherwise send the cue to its first use.
    if let Some(at) = clips.get(cue.cut).and_then(here) {
        return Some((cue.cut, at));
    }
    clips
        .iter()
        .enumerate()
        .find_map(|(index, clip)| here(clip).map(|at| (index, at)))
}

/// The most voice-over lines and sound effects one edit lays.
pub(crate) const MAX_CUES: usize = 40;

/// The `filter_complex` that lays cues, given as (start in seconds, whether it is a voice) for
/// inputs `1..`, over input 0's audio into `[audio]`. Voices are levelled like speech, and the
/// footage's own sound gives way under them; effects sit below speech, cut after a few seconds.
fn cue_mix(cues: &[(f64, bool)]) -> String {
    let stereo = "aformat=sample_rates=48000:channel_layouts=stereo";
    let mut graph = format!("[0:a:0]{stereo}[base];");
    let (mut voices, mut effects) = (Vec::new(), Vec::new());
    for (index, &(at, voice)) in cues.iter().enumerate() {
        let delay = (at.max(0.0) * 1000.0).round() as u64;
        let level = if voice {
            "loudnorm=I=-16:TP=-2:LRA=11,aresample=48000".to_owned()
        } else {
            format!(
                "atrim=0:{seconds},afade=t=out:st={fade}:d=0.5,volume=-8dB",
                seconds = crate::motion_design::MAX_SOUND_SECONDS,
                fade = crate::motion_design::MAX_SOUND_SECONDS - 0.5,
            )
        };
        let label = format!("c{index}");
        graph.push_str(&format!(
            "[{input}:a:0]{stereo},{level},{stereo},adelay=delays={delay}:all=1[{label}];",
            input = index + 1
        ));
        if voice {
            voices.push(label);
        } else {
            effects.push(label);
        }
    }
    let mut parts = Vec::new();
    if voices.is_empty() {
        parts.push("[base]".to_owned());
    } else {
        let joined: String = voices.iter().map(|label| format!("[{label}]")).collect();
        graph.push_str(&format!(
            // Padded with silence: the compressor stops when its key ends, which would cut the
            // footage's sound short after the last line.
            "{joined}amix=inputs={}:normalize=0:duration=longest,apad,asplit=2[voice][key];\
             [base][key]sidechaincompress=threshold=0.02:ratio=8:attack=20:release=400[ducked];",
            voices.len()
        ));
        parts.extend(["[ducked]".to_owned(), "[voice]".to_owned()]);
    }
    parts.extend(effects.iter().map(|label| format!("[{label}]")));
    graph.push_str(&format!(
        "{}amix=inputs={}:normalize=0:duration=first[audio]",
        parts.concat(),
        parts.len()
    ));
    graph
}

/// The music a storyboard lays under its scenes: a first track, the tracks it changes to at
/// scenes, and whether the music is featured rather than kept in the background.
struct SceneMusic<'a> {
    first: Option<&'a Path>,
    changes: &'a [(PathBuf, usize)],
    featured: bool,
}

/// Removes silence from both ends of a track, keeping 50 ms before and after the sound.
const TRIM_SILENCE: &str = "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05,areverse,\
     silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05,areverse";

/// Seconds two music tracks overlap where the music changes.
const MUSIC_CROSSFADE: f64 = 2.0;

/// The filters that make the `[bed]` music stream for a `length`-second video from inputs
/// `1..=starts.len()`, each looped, levelled, and starting at its start time with a crossfade.
fn music_bed(starts: Vec<f64>, length: f64, stereo: &str, featured: bool) -> String {
    let level = if featured {
        "loudnorm=I=-19:TP=-3:LRA=9"
    } else {
        "loudnorm=I=-30:TP=-9:LRA=7"
    };
    if starts.len() <= 1 {
        return format!("[1:a:0]{stereo},{level}[bed];");
    }
    // Each track plays from its own start until the next one has faded in.
    let mut ends: Vec<f64> = starts[1..].to_vec();
    ends.push(length + 1.0);
    let shortest = starts
        .iter()
        .zip(&ends)
        .map(|(start, end)| end - start)
        .fold(f64::INFINITY, f64::min);
    let fade = MUSIC_CROSSFADE.min(shortest / 2.0).max(0.1);
    let mut graph = String::new();
    let last = starts.len() - 1;
    for (index, (start, end)) in starts.iter().zip(&ends).enumerate() {
        let before = if index == 0 { 0.0 } else { fade / 2.0 };
        let after = if index == last { 0.0 } else { fade / 2.0 };
        let seconds = end - start + before + after;
        graph.push_str(&format!(
            "[{input}:a:0]{stereo},atrim=duration={seconds:.3},asetpts=PTS-STARTPTS,{level}[m{index}];",
            input = index + 1
        ));
    }
    let mut current = "m0".to_owned();
    for index in 1..starts.len() {
        let label = if index == last {
            "bed".to_owned()
        } else {
            format!("x{index}")
        };
        graph.push_str(&format!(
            "[{current}][m{index}]acrossfade=d={fade:.3}:c1=qsin:c2=qsin[{label}];"
        ));
        current = label;
    }
    graph
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
    asset_id: i64,
    path: PathBuf,
    cut: Option<PreviewCut>,
    has_audio: bool,
}

const LOST_LEASE: &str = "The render stopped because its run was stopped or taken over";

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
            Some(scenes) => narration_tracks(database, workspace_id, run_id, &edit, scenes.len())
                .step("Finding the narration audio")?,
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
        let motion = self
            .edit_motion(workspace_id, &edit)
            .step("Reading the animations")?;
        let cues = self
            .edit_cues(database, workspace_id, run_id, &edit)
            .step("Finding the sound effects")?;
        let music = self
            .edit_music(&asset_root(database), workspace_id, &edit)
            .step("Finding the music")?;
        let music_changes = self
            .edit_music_changes(&asset_root(database), workspace_id, &edit)
            .step("Finding the music changes")?;

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
            fs::create_dir_all(&delivery).step("Creating the delivery folder")?;
            let delivery = delivery
                .canonicalize()
                .step("Resolving the delivery folder")?;
            let scratch = delivery.join(format!("render-attempt-{attempt}"));
            fs::create_dir(&scratch).step("Creating the render folder")?;
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
                    let check = renderer
                        .render_scenes(
                            &root,
                            &scratch,
                            scenes,
                            &narration,
                            &format,
                            SceneMusic {
                                first: music.as_deref(),
                                changes: &music_changes,
                                featured: edit["music_level"] == "featured",
                            },
                        )
                        .step("Rendering the storyboard scenes")?;
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
                let clips_index =
                    if let Some(index) = clip_sets.iter().position(|(cached, _)| cached == &cuts) {
                        index
                    } else {
                        let clips = renderer
                            .inspect_clips(&root, &inputs.draft.source_ids, cuts.as_deref())
                            .step("Inspecting the source clips")?;
                        clip_sets.push((cuts, clips));
                        clip_sets.len() - 1
                    };
                let check = renderer
                    .render_format(
                        &root,
                        &scratch,
                        &clip_sets[clips_index].1,
                        &format,
                        &motion,
                        music.as_deref(),
                        &cues,
                    )
                    .step("Rendering the cut")?;
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
            let music = self.edit_music(&root, workspace_id, &edit)?;
            let changes = self.edit_music_changes(&root, workspace_id, &edit)?;
            renderer.render_scenes(
                &root,
                directory,
                &scenes,
                &narration,
                &format,
                SceneMusic {
                    first: music.as_deref(),
                    changes: &changes,
                    featured: edit["music_level"] == "featured",
                },
            )?;
        } else {
            let source_ids = &inputs.draft.source_ids;
            let cuts = preview_cuts(&edit, source_ids)?;
            let clips = renderer.inspect_clips(&root, source_ids, cuts.as_deref())?;
            let motion = self.edit_motion(workspace_id, &edit)?;
            let music = self.edit_music(&root, workspace_id, &edit)?;
            let cues = self.edit_cues(database, workspace_id, run_id, &edit)?;
            renderer.render_format(
                &root,
                directory,
                &clips,
                &format,
                &motion,
                music.as_deref(),
                &cues,
            )?;
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
        // Animations may show the workspace's clips and play its sounds, like storyboard scenes.
        let media = |kind: &str| -> rusqlite::Result<Vec<i64>> {
            self.0
                .prepare(
                    "SELECT id FROM workspace_assets
                     WHERE workspace_id = ?1 AND kind = ?2 AND superseded = 0",
                )?
                .query_map(params![workspace_id, kind], |row| row.get(0))?
                .collect()
        };
        let (clips, sounds) = (media("video")?, media("audio")?);
        let animations = animations
            .iter()
            .take(crate::motion_design::MAX_RUN_ANIMATIONS * 4)
            .map(|spec| {
                MotionSpec::from_answer_with_media(
                    spec,
                    |id| images.contains(&id),
                    |id| clips.contains(&id),
                    |id| sounds.contains(&id),
                )
            })
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

    /// The voice-over lines and sound effects an edit of recorded footage lays, validated: sound
    /// effects are audio in this workspace and voice-over files are ones Arrumo wrote for the run.
    fn edit_cues(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        edit: &Value,
    ) -> Result<Vec<AudioCue>, Box<dyn Error>> {
        let root = asset_root(database);
        let at = |entry: &Value| entry["at_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
        let cut = |entry: &Value| entry["cut"].as_u64().map(|cut| cut as usize);
        // The moment in the source footage a cue lands on, from the main cut it names.
        let source = |entry: &Value| -> Option<(i64, u32)> {
            let main = &edit["cuts"][entry["cut"].as_u64()? as usize];
            let (start, end) = (main["start_ms"].as_u64()?, main["end_ms"].as_u64()?);
            let moment = (start + entry["at_ms"].as_u64().unwrap_or(0)).min(end.saturating_sub(1));
            Some((main["asset_id"].as_i64()?, u32::try_from(moment).ok()?))
        };
        let mut cues = Vec::new();
        for entry in edit["sound_cues"].as_array().into_iter().flatten() {
            let (Some(cut), Some(asset)) = (cut(entry), entry["asset_id"].as_i64()) else {
                return Err(io::Error::other("A sound effect in the edit is incomplete").into());
            };
            let path = self.music_track(&root, workspace_id, &json!({"asset_id": asset}))?;
            cues.push(AudioCue {
                cut,
                at: at(entry),
                path,
                voice: false,
                source: source(entry),
            });
        }
        for entry in edit["voice_cues"].as_array().into_iter().flatten() {
            let (Some(cut), Some(file)) = (cut(entry), entry["file"].as_str()) else {
                continue;
            };
            if !crate::narration::voice_over_file_name(file) {
                return Err(
                    io::Error::other("A voice-over file name is not one Arrumo wrote").into(),
                );
            }
            let path = crate::delivery_verification::managed_delivery_file(
                database,
                workspace_id,
                run_id,
                file,
            )?;
            cues.push(AudioCue {
                cut,
                at: at(entry),
                path,
                voice: true,
                source: source(entry),
            });
        }
        if cues.len() > MAX_CUES {
            return Err(io::Error::other("The edit lays too many sounds").into());
        }
        Ok(cues)
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
        self.music_track(root, workspace_id, music).map(Some)
    }

    /// The workspace audio file a music entry (`{"asset_id": …}`) names.
    fn music_track(
        &self,
        root: &Path,
        workspace_id: i64,
        music: &Value,
    ) -> Result<PathBuf, Box<dyn Error>> {
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
        Ok(asset_path(root, workspace_id, id)
            .canonicalize()
            .io_step("Finding an audio track's file")?)
    }

    /// The tracks a storyboard edit changes its music to, each with the scene it starts on.
    fn edit_music_changes(
        &self,
        root: &Path,
        workspace_id: i64,
        edit: &Value,
    ) -> Result<Vec<(PathBuf, usize)>, Box<dyn Error>> {
        let Some(changes) = edit.get("music_changes").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        changes
            .iter()
            .map(|change| {
                let scene = change["scene"].as_u64().unwrap_or(0) as usize;
                Ok((self.music_track(root, workspace_id, change)?, scene))
            })
            .collect()
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
        let is_clip = |id: i64| {
            self.0
                .query_row(
                    "SELECT 1 FROM workspace_assets
                     WHERE workspace_id = ?1 AND id = ?2 AND kind = 'video' AND superseded = 0",
                    params![workspace_id, id],
                    |_| Ok(()),
                )
                .is_ok()
        };
        let is_sound = |id: i64| {
            self.0
                .query_row(
                    "SELECT 1 FROM workspace_assets
                     WHERE workspace_id = ?1 AND id = ?2 AND kind = 'audio' AND superseded = 0",
                    params![workspace_id, id],
                    |_| Ok(()),
                )
                .is_ok()
        };
        scenes
            .iter()
            .map(|scene| {
                MotionSpec::from_answer_with_media(
                    scene,
                    |id| images.contains(&id),
                    is_clip,
                    is_sound,
                )
                .map_err(|reason| io::Error::other(reason).into())
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()
            .map(|mut scenes| {
                crate::motion_spec::continue_clips(&mut scenes);
                Some(scenes)
            })
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
        music: SceneMusic<'_>,
    ) -> Result<RenderCheck, Box<dyn Error>> {
        let directory = scratch.join(format!("scenes-{}", format.id));
        fs::create_dir(&directory).io_step("Creating a folder")?;
        let images = |id: i64| asset_path(root, self.workspace_id, id).canonicalize().ok();
        let fonts = self
            .store
            .run_font_set(root, self.workspace_id, self.run_id)?;
        let mut renewed = Instant::now();
        let mut check = || self.keep_lease(&mut renewed);
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
                &fonts,
                narration.get(index).and_then(|(audio, _)| audio.as_deref()),
                &mut check,
            )?;
            parts.push(probe_duration(&directory.join(&name))?);
            manifest.push_str(&format!("file {name}\n"));
        }
        fs::write(directory.join("scenes.ffconcat"), manifest).io_step("Writing a file")?;
        let output = scratch.join(format!("{}-joined.mp4", format.id));
        let transitions: Vec<_> = scenes.iter().map(|scene| scene.transition).collect();
        let args: Vec<String> = if transitions[..transitions.len().saturating_sub(1)]
            .iter()
            .any(|transition| !transition.is_none())
        {
            // Pictures are joined through the transitions; the soundtrack is the plain join.
            let mut args: Vec<String> = ["-nostdin", "-v", "error", "-protocol_whitelist", "file"]
                .map(String::from)
                .to_vec();
            for index in 0..scenes.len() {
                args.extend(["-i".to_owned(), format!("scene-{index}.mp4")]);
            }
            args.extend(
                [
                    "-f",
                    "concat",
                    "-safe",
                    "1",
                    "-i",
                    "scenes.ffconcat",
                    "-filter_complex",
                ]
                .map(String::from),
            );
            args.push(crate::scene_transitions::join_graph(
                &parts,
                &transitions,
                format.width,
                format.height,
            ));
            args.extend(["-map".into(), "[out]".into(), "-map".into()]);
            args.push(format!("{}:a:0", scenes.len()));
            args.extend(
                [
                    "-r",
                    "30",
                    "-c:v",
                    "libx264",
                    "-crf",
                    "17",
                    "-preset",
                    "medium",
                    "-threads",
                    "2",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "copy",
                    "-movflags",
                    "+faststart",
                    "-y",
                ]
                .map(String::from),
            );
            args.push(output.to_string_lossy().into_owned());
            args
        } else {
            [
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
            .collect()
        };
        run_ffmpeg(&directory, &args, &mut check)?;
        let finished = scratch.join(format!("{}.mp4", format.id));
        let staged = scratch.join(format!("{}-staged.mp4", format.id));
        // Each music track starts where its scene starts.
        let starts: Vec<f64> = parts
            .iter()
            .scan(0.0, |elapsed, length| {
                let start = *elapsed;
                *elapsed += length;
                Some(start)
            })
            .collect();
        let tracks: Vec<(&Path, f64)> = music
            .first
            .map(|first| (first, 0.0))
            .into_iter()
            .chain(music.changes.iter().filter_map(|(path, scene)| {
                starts
                    .get(*scene)
                    .filter(|_| *scene > 0)
                    .map(|start| (path.as_path(), *start))
            }))
            .collect();
        let voiced = narration.iter().any(|(audio, _)| audio.is_some());
        self.add_soundtrack(&output, &staged, &finished, &tracks, music.featured, voiced)?;
        self.check_render(&finished, &parts)
    }

    /// Masters the joined scenes' audio with their music into `finished`. `master_audio` uses up
    /// its input, so only a file it did not read is removed here.
    fn add_soundtrack(
        &self,
        joined: &Path,
        staged: &Path,
        finished: &Path,
        tracks: &[(&Path, f64)],
        featured: bool,
        voiced: bool,
    ) -> Result<(), Box<dyn Error>> {
        match tracks.is_empty() {
            // With a voice, the music sits under the mastered narration as it does under speech.
            false if voiced => {
                self.master_audio(joined, staged)?;
                self.lay_music_tracks(staged, tracks, featured, finished)?;
                fs::remove_file(staged).io_step("Removing a file")?;
            }
            // Without one, the music is the whole soundtrack, so it is mastered to full level.
            false => {
                self.lay_music_tracks(joined, tracks, featured, staged)?;
                self.master_audio(staged, finished)?;
                fs::remove_file(joined).io_step("Removing a file")?;
            }
            true => self.master_audio(joined, finished)?,
        }
        Ok(())
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
                if !path
                    .symlink_metadata()
                    .io_step("Checking a file")?
                    .is_file()
                {
                    return Err(io::Error::other("A managed source video is missing").into());
                }
                let path = path.canonicalize().io_step("Resolving a path")?;
                let has_audio = self.inspect_source(&path, cut)?;
                Ok(RenderClip {
                    asset_id: id,
                    path,
                    cut: cut.copied(),
                    has_audio,
                })
            })
            .collect()
    }

    fn confirm_lease(&self) -> Result<(), Box<dyn Error>> {
        let held = self
            .store
            .0
            .query_row(
                "SELECT id FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
                 AND state = 'running' AND lease_until > unixepoch()",
                params![self.workspace_id, self.run_id, self.attempt],
                |_| Ok(()),
            )
            .optional()?;
        held.ok_or_else(|| self.lost_lease())
    }

    /// Why the lease is gone, from the run as it is now, so a report can tell a stop, a takeover
    /// and a lapse apart.
    fn lost_lease(&self) -> Box<dyn Error> {
        let now: Option<(String, i64, Option<i64>)> = self
            .store
            .0
            .query_row(
                "SELECT state, attempt, unixepoch() - lease_until FROM generation_runs
                 WHERE workspace_id = ?1 AND id = ?2",
                params![self.workspace_id, self.run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .ok()
            .flatten();
        let detail = match now {
            None => "the run no longer exists".to_owned(),
            Some((state, attempt, lapsed)) => {
                let lease = lapsed.map_or("no lease".to_owned(), |seconds| {
                    if seconds >= 0 {
                        format!("lease lapsed {seconds} s ago")
                    } else {
                        format!("lease held for {} s more", -seconds)
                    }
                });
                format!(
                    "run {state}, attempt {attempt} (this render is attempt {}), {lease}",
                    self.attempt
                )
            }
        };
        io::Error::other(format!("{LOST_LEASE} ({detail})")).into()
    }

    /// Renews the lease every ten seconds and confirms it in between, for any step that runs
    /// longer than the lease, such as rendering an animation.
    fn keep_lease(&self, renewed: &mut Instant) -> Result<(), Box<dyn Error>> {
        if renewed.elapsed() < Duration::from_secs(10) {
            return self.confirm_lease();
        }
        *renewed = Instant::now();
        self.renew_lease()
    }

    /// Renews the lease now, before a step that may follow a long one.
    fn renew_lease(&self) -> Result<(), Box<dyn Error>> {
        self.store
            .renew_generation_lease(self.workspace_id, self.run_id, self.attempt)
            .optional()?
            .map(drop)
            .ok_or_else(|| self.lost_lease())
    }

    fn run(&self, command: &mut Command, operation: &str) -> Result<(), Box<dyn Error>> {
        self.run_output(command, operation).map(drop)
    }

    /// Runs FFmpeg while keeping the lease, returning what it wrote to stderr.
    fn run_output(&self, command: &mut Command, operation: &str) -> Result<String, Box<dyn Error>> {
        self.renew_lease()?;
        let mut heartbeat = Instant::now();
        let mut process = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .launch()?;
        // FFmpeg runs with `-v error`, so its error output stays small.
        let errors = process.stderr.take().map(|mut stderr| {
            thread::spawn(move || {
                let mut text = String::new();
                let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
                text
            })
        });
        loop {
            if let Some(status) = process.try_wait()? {
                let errors = errors
                    .and_then(|reader| reader.join().ok())
                    .unwrap_or_default();
                return if status.success() {
                    Ok(errors)
                } else {
                    Err(crate::quiet_process::ffmpeg_failure(
                        &format!("FFmpeg failed while {operation} ({status})"),
                        &errors,
                        command.get_args(),
                    )
                    .into())
                };
            }
            if let Err(error) = self.keep_lease(&mut heartbeat) {
                let _ = process.kill();
                let _ = process.wait();
                return Err(error);
            }
            thread::sleep(Duration::from_millis(250));
        }
    }

    /// Writes `input` to `output` with its audio at the delivery loudness, measured first so
    /// the gain is linear. A silent or nearly silent video is moved unchanged.
    fn master_audio(&self, input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        let measured = self.run_output(
            crate::quiet_command(crate::media_tools::ffmpeg())
                .args(["-nostdin", "-hide_banner", "-nostats", "-v", "info"])
                .args(["-protocol_whitelist", "file", "-i"])
                .arg(input)
                .args(["-map", "0:a:0", "-af"])
                .arg(format!("loudnorm={MASTER_LOUDNESS}:print_format=json"))
                .args(["-f", "null", "-"]),
            "measuring loudness",
        )?;
        let Some(filter) = loudness_pass(&measured) else {
            fs::rename(input, output).io_step("Moving a file")?;
            return Ok(());
        };
        self.run(
            crate::quiet_command(crate::media_tools::ffmpeg())
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
        fs::remove_file(input).io_step("Removing a file")?;
        Ok(())
    }

    fn inspect_source(
        &self,
        path: &Path,
        cut: Option<&PreviewCut>,
    ) -> Result<bool, Box<dyn Error>> {
        self.renew_lease()?;
        let output = crate::quiet_command(crate::media_tools::ffprobe())
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
            .launch_output()?;
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

    #[allow(clippy::too_many_arguments)]
    fn render_format(
        &self,
        root: &Path,
        scratch: &Path,
        clips: &[RenderClip],
        format: &Dimensions,
        motion: &EditMotion,
        music: Option<&Path>,
        cues: &[AudioCue],
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
        let fonts = self
            .store
            .run_font_set(root, self.workspace_id, self.run_id)?;
        let mut renewed = Instant::now();
        let mut check = || self.keep_lease(&mut renewed);
        let mut parts = Vec::new();
        let mut insert = |cut: usize,
                          manifest: &mut String,
                          parts: &mut Vec<f64>|
         -> Result<(), Box<dyn Error>> {
            for (number, spec) in motion.inserts_before(cut).enumerate() {
                let name = format!("insert-{id}-{cut}-{number}.mp4");
                render_spec(
                    spec,
                    (*width, *height),
                    scratch,
                    &name,
                    &images,
                    &fonts,
                    &mut check,
                )?;
                parts.push(probe_duration(&scratch.join(&name))?);
                manifest.push_str(&format!("file {name}\n"));
            }
            Ok(())
        };
        let mut spans = Vec::with_capacity(clips.len());
        for (index, clip) in clips.iter().enumerate() {
            insert(index, &mut manifest, &mut parts)?;
            let start: f64 = parts.iter().sum();
            let filename = format!("clip-{id}-{index}.mp4");
            let mut command = crate::quiet_command(crate::media_tools::ffmpeg());
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
            let grade = clip
                .cut
                .and_then(|cut| cut.grade)
                .and_then(Grade::clamped)
                .map(Grade::filter)
                .unwrap_or_default();
            let mut graph = format!("[0:v]{grade}{filter}[base];");
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
                    &fonts,
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
            let length = probe_duration(&scratch.join(&filename))?;
            spans.push((start, length));
            parts.push(length);
            manifest.push_str(&format!("file {filename}\n"));
        }
        insert(clips.len(), &mut manifest, &mut parts)?;
        let list = scratch.join(format!("{id}.ffconcat"));
        fs::write(&list, manifest).io_step("Writing a file")?;
        let mut command = crate::quiet_command(crate::media_tools::ffmpeg());
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
        let mut joined = scratch.join(format!("{id}-joined.mp4"));
        // A cue starts inside its cut, at least half a second before the cut ends.
        let placed: Vec<(f64, &AudioCue)> = cues
            .iter()
            .take(MAX_CUES)
            .filter_map(|cue| {
                let (cut, at) = cue_position(cue, clips)?;
                let &(start, length) = spans.get(cut)?;
                Some((start + at.clamp(0.0, (length - 0.5).max(0.0)), cue))
            })
            .collect();
        if !placed.is_empty() {
            let cued = scratch.join(format!("{id}-cued.mp4"));
            self.lay_cues(&joined, &placed, &cued)?;
            joined = cued;
        }
        match music {
            Some(music) => {
                let mastered = scratch.join(format!("{id}-mastered.mp4"));
                self.master_audio(&joined, &mastered)?;
                self.lay_music(&mastered, music, &output)?;
                fs::remove_file(mastered).io_step("Removing a file")?;
            }
            None => self.master_audio(&joined, &output)?,
        }
        self.check_render(&output, &parts)
    }

    /// Lays voice-over lines and sound effects, each at its start in seconds, over `input`'s audio.
    fn lay_cues(
        &self,
        input: &Path,
        cues: &[(f64, &AudioCue)],
        output: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let mut command = crate::quiet_command(crate::media_tools::ffmpeg());
        command
            .args([
                "-nostdin",
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-i",
            ])
            .arg(input);
        for (_, cue) in cues {
            command.arg("-i").arg(&cue.path);
        }
        let timing: Vec<(f64, bool)> = cues.iter().map(|(at, cue)| (*at, cue.voice)).collect();
        self.run(
            command
                .arg("-filter_complex")
                .arg(cue_mix(&timing))
                .args(["-map", "0:v:0", "-map", "[audio]", "-c:v", "copy"])
                .args(["-c:a", "aac", "-ar", "48000", "-ac", "2", "-y"])
                .arg(output),
            "laying voice-over and sound effects",
        )
    }

    /// Loops `music` under the mastered speech at a background level, lowered about 10 dB while
    /// anyone speaks and faded out over the last 1.5 seconds, then limited to the delivery peak.
    fn lay_music(&self, input: &Path, music: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        self.lay_music_tracks(input, &[(music, 0.0)], false, output)
    }

    /// Like `lay_music`, changing to each further track at its start time (in seconds) with a
    /// crossfade. Each track is levelled on its own so the changes keep one loudness.
    fn trim_silence(&self, track: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
        self.run(
            crate::quiet_command(crate::media_tools::ffmpeg())
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file",
                    "-i",
                ])
                .arg(track)
                .args(["-map", "0:a:0", "-af"])
                .arg(TRIM_SILENCE)
                .args(["-c:a", "pcm_s16le", "-y"])
                .arg(output),
            "trimming silence from a music track",
        )
    }

    fn lay_music_tracks(
        &self,
        input: &Path,
        tracks: &[(&Path, f64)],
        featured: bool,
        output: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let length = probe_duration(input)?;
        let fade_at = (length - 1.5).max(0.0);
        let stereo = "aformat=sample_rates=48000:channel_layouts=stereo";
        let mut command = crate::quiet_command(crate::media_tools::ffmpeg());
        command
            .args([
                "-nostdin",
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-i",
            ])
            .arg(input);
        // Generated tracks often end in silence, which would leave gaps where they loop or hand over.
        let trimmed = tempfile::tempdir()?;
        for (index, (track, _)) in tracks.iter().enumerate() {
            let sound = trimmed.path().join(format!("{index}.wav"));
            self.trim_silence(track, &sound)?;
            command.args(["-stream_loop", "-1", "-i"]).arg(sound);
        }
        let bed = music_bed(
            tracks.iter().map(|(_, start)| *start).collect(),
            length,
            stereo,
            featured,
        );
        // Featured music sits nearer the voice's level and gives way further while anyone speaks.
        let duck = if featured {
            "threshold=0.015:ratio=20:attack=15:release=350"
        } else {
            "threshold=0.02:ratio=8:attack=20:release=400"
        };
        self.run(
            command
                .arg("-filter_complex")
                .arg(format!(
                    "{bed}\
                     [0:a:0]{stereo},asplit=2[speech][key];\
                     [bed][key]sidechaincompress={duck}[ducked];\
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
        let joins = parts
            .iter()
            .scan(0.0, |at, part| {
                *at += part;
                Some(*at)
            })
            .take(parts.len().saturating_sub(1))
            .collect();
        Ok(RenderCheck {
            seconds,
            expected_seconds,
            joins,
        })
    }
}

fn probe_duration(path: &Path) -> Result<f64, Box<dyn Error>> {
    let output = crate::quiet_command(crate::media_tools::ffprobe())
        .args(["-v", "error", "-protocol_whitelist", "file"])
        .args(["-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(path)
        .launch_output()
        .map_err(|error| {
            io::Error::new(error.kind(), format!("ffprobe could not start: {error}"))
        })?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|seconds| output.status.success() && seconds.is_finite())
        .ok_or_else(|| {
            let errors = String::from_utf8_lossy(&output.stderr);
            io::Error::other(format!(
                "Could not measure a rendered video's duration ({}): {}",
                output.status,
                errors.lines().rev().take(4).collect::<Vec<_>>().join(" | ")
            ))
            .into()
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn music_loses_the_silence_at_both_ends() {
        if crate::quiet_command(crate::media_tools::ffmpeg())
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("trimmed.wav");
        let made = crate::quiet_command(crate::media_tools::ffmpeg())
            .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
            .arg("anullsrc=r=48000:cl=stereo:d=1[a];sine=f=440:d=2,aformat=channel_layouts=stereo[b];anullsrc=r=48000:cl=stereo:d=1.5[c];[a][b][c]concat=n=3:v=0:a=1")
            .arg("-af")
            .arg(super::TRIM_SILENCE)
            .arg(&output)
            .status()
            .unwrap();
        assert!(made.success());
        let seconds = crate::source_transcripts::media_duration(&output).unwrap();
        assert!((seconds - 2.1).abs() < 0.08, "{seconds}");
    }

    #[test]
    fn storyboard_scenes_get_their_music_with_or_without_a_voice() {
        use crate::{DraftSettings, RunOptions, WorkspaceStore};
        let dir = tempfile::tempdir().unwrap();
        let make = |path: &std::path::Path, args: &[&str]| {
            crate::quiet_command(crate::media_tools::ffmpeg())
                .args(["-nostdin", "-v", "error"])
                .args(args)
                .arg(path)
                .status()
                .is_ok_and(|status| status.success())
        };
        let scenes = [
            "-f",
            "lavfi",
            "-i",
            "color=c=gray:s=160x90:d=2",
            "-f",
            "lavfi",
            "-i",
            "sine=f=220:d=2",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-y",
        ];
        let music = dir.path().join("music.wav");
        if !make(&music, &["-f", "lavfi", "-i", "sine=f=660:d=2", "-y"]) {
            eprintln!("FFmpeg is unavailable; skipping the soundtrack check");
            return;
        }
        let database = dir.path().join("arrumo.db");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let profile = store
            .create_profile(workspace, "Shorts", "", None)
            .unwrap()
            .id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Story".into(),
                    intent: "A storyboard".into(),
                    profile_id: Some(profile),
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap()
            .id;
        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let renderer = super::ActiveRender {
            store: &store,
            workspace_id: workspace,
            run_id: run,
            attempt,
        };
        for voiced in [false, true] {
            let joined = dir.path().join(format!("joined-{voiced}.mp4"));
            let staged = dir.path().join(format!("staged-{voiced}.mp4"));
            let finished = dir.path().join(format!("finished-{voiced}.mp4"));
            assert!(make(&joined, &scenes));
            renderer
                .add_soundtrack(&joined, &staged, &finished, &[(&music, 0.0)], false, voiced)
                .unwrap();
            assert!(finished.is_file(), "voiced: {voiced}");
            assert!(!staged.exists() && !joined.exists(), "voiced: {voiced}");
        }
    }

    #[test]
    fn a_render_longer_than_the_lease_keeps_it() {
        use crate::{DraftSettings, RunOptions, WorkspaceStore};
        use std::time::Instant;
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Story".into(),
                    intent: "A long animation".into(),
                    profile_id: None,
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap()
            .id;
        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let renderer = super::ActiveRender {
            store: &store,
            workspace_id: workspace,
            run_id: run,
            attempt,
        };
        let lapse = || {
            store
                .0
                .execute(
                    "UPDATE generation_runs SET lease_until = unixepoch() - 1 WHERE id = ?1",
                    [run],
                )
                .unwrap();
        };
        // A step that outlived the lease renews it rather than failing.
        lapse();
        renderer.renew_lease().unwrap();
        let mut renewed = Instant::now();
        renderer.keep_lease(&mut renewed).unwrap();
        // Between renewals a lapsed lease says so in words.
        lapse();
        let lost = renderer.keep_lease(&mut renewed).unwrap_err();
        assert!(lost.to_string().contains("lease lapsed"), "{lost}");
        assert!(lost.to_string().starts_with(super::LOST_LEASE), "{lost}");
        // Another attempt took over, so this one stops.
        store
            .0
            .execute(
                "UPDATE generation_runs SET attempt = attempt + 1 WHERE id = ?1",
                [run],
            )
            .unwrap();
        let lost = renderer.renew_lease().unwrap_err();
        assert!(lost.to_string().starts_with(super::LOST_LEASE), "{lost}");
        assert!(
            lost.to_string()
                .contains(&format!("attempt {}", attempt + 1)),
            "{lost}"
        );
    }

    #[test]
    fn music_changes_crossfade_at_their_scenes() {
        if crate::quiet_command(crate::media_tools::ffmpeg())
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let mut args: Vec<String> = [
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=48000:cl=stereo:d=9",
        ]
        .map(String::from)
        .to_vec();
        for frequency in [220, 440, 880] {
            let path = directory.path().join(format!("{frequency}.wav"));
            let made = crate::quiet_command(crate::media_tools::ffmpeg())
                .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
                .arg(format!("sine=f={frequency}:d=2"))
                .arg(&path)
                .status()
                .unwrap();
            assert!(made.success());
            args.extend([
                "-stream_loop".into(),
                "-1".into(),
                "-i".into(),
                path.to_string_lossy().into(),
            ]);
        }
        let stereo = "aformat=sample_rates=48000:channel_layouts=stereo";
        let bed = super::music_bed(vec![0.0, 3.0, 6.0], 9.0, stereo, false);
        let output = directory.path().join("bed.wav");
        args.extend([
            "-filter_complex".into(),
            format!("{bed}[0:a][bed]amix=inputs=2:duration=first[out]"),
        ]);
        args.extend([
            "-map".into(),
            "[out]".into(),
            "-y".into(),
            output.to_string_lossy().into(),
        ]);
        assert!(
            crate::quiet_command(crate::media_tools::ffmpeg())
                .args(&args)
                .status()
                .unwrap()
                .success()
        );
        let seconds = crate::source_transcripts::media_duration(&output).unwrap();
        assert!((seconds - 9.0).abs() < 0.05, "{seconds}");
        // Each track is heard in its own stretch.
        let pitch_at = |from: f64| {
            let report = crate::quiet_command(crate::media_tools::ffmpeg())
                .args([
                    "-nostdin",
                    "-v",
                    "info",
                    "-ss",
                    &from.to_string(),
                    "-t",
                    "0.5",
                    "-i",
                ])
                .arg(&output)
                .args(["-af", "aresample=8000,astats=metadata=0", "-f", "null", "-"])
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&report.stderr).into_owned();
            text.lines()
                .find_map(|line| line.split("Zero crossings rate: ").nth(1))
                .and_then(|value| value.trim().parse::<f64>().ok())
                .unwrap()
        };
        let (first, second, third) = (pitch_at(1.0), pitch_at(4.5), pitch_at(7.5));
        assert!(first < second && second < third, "{first} {second} {third}");
    }

    use super::{cut_audio_filter, duration_matches};

    #[test]
    fn cues_duck_the_footage_under_voice_and_keep_its_length() {
        let graph = super::cue_mix(&[(1.0, true), (0.25, false)]);
        assert!(graph.contains("[1:a:0]") && graph.contains("adelay=delays=1000:all=1[c0]"));
        assert!(graph.contains("adelay=delays=250:all=1[c1]"));
        assert!(graph.contains("[base][key]sidechaincompress"));
        assert!(
            graph.ends_with("[ducked][voice][c1]amix=inputs=3:normalize=0:duration=first[audio]")
        );
        let effects_only = super::cue_mix(&[(0.0, false)]);
        assert!(!effects_only.contains("sidechaincompress"));
        assert!(
            effects_only.ends_with("[base][c0]amix=inputs=2:normalize=0:duration=first[audio]")
        );

        // FFmpeg accepts the graph, and the result keeps the footage's length.
        let ffmpeg = crate::media_tools::ffmpeg();
        let dir = tempfile::tempdir().unwrap();
        let make = |name: &str, args: &[&str]| {
            let status = std::process::Command::new(ffmpeg)
                .args(["-nostdin", "-v", "error"])
                .args(args)
                .arg(dir.path().join(name))
                .status();
            status.is_ok_and(|status| status.success())
        };
        if !make(
            "footage.mp4",
            &[
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=160x90:d=3",
                "-f",
                "lavfi",
                "-i",
                "sine=f=220:d=3",
                "-shortest",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-y",
            ],
        ) {
            eprintln!("FFmpeg is unavailable; skipping the mix check");
            return;
        }
        assert!(make(
            "voice.wav",
            &["-f", "lavfi", "-i", "sine=f=440:d=1.5", "-y"]
        ));
        assert!(make(
            "effect.wav",
            &["-f", "lavfi", "-i", "sine=f=1800:d=0.2", "-y"]
        ));
        let status = std::process::Command::new(ffmpeg)
            .args(["-nostdin", "-v", "error", "-i"])
            .arg(dir.path().join("footage.mp4"))
            .arg("-i")
            .arg(dir.path().join("voice.wav"))
            .arg("-i")
            .arg(dir.path().join("effect.wav"))
            .arg("-filter_complex")
            .arg(&graph)
            .args([
                "-map", "0:v:0", "-map", "[audio]", "-c:v", "copy", "-c:a", "aac", "-y",
            ])
            .arg(dir.path().join("mixed.mp4"))
            .status()
            .unwrap();
        assert!(status.success());
        // The audio itself, not just the container, lasts past the last voice line.
        let probe = std::process::Command::new(crate::media_tools::ffprobe())
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=duration",
            ])
            .args(["-of", "csv=p=0"])
            .arg(dir.path().join("mixed.mp4"))
            .output()
            .unwrap();
        let seconds: f64 = String::from_utf8_lossy(&probe.stdout)
            .trim()
            .parse()
            .unwrap();
        assert!((seconds - 3.0).abs() < 0.2, "{seconds}");
    }

    #[test]
    fn a_preview_lays_voice_over_and_sound_effects_over_its_footage() {
        use crate::{DraftSettings, RunOptions, WorkspaceStore, generation_plan::ProductionStage};
        use serde_json::json;
        use std::path::Path;
        let ffmpeg = crate::media_tools::ffmpeg();
        let dir = tempfile::tempdir().unwrap();
        let make = |path: &Path, args: &[&str]| {
            std::process::Command::new(ffmpeg)
                .args(["-nostdin", "-v", "error"])
                .args(args)
                .arg(path)
                .status()
                .is_ok_and(|status| status.success())
        };
        let footage = dir.path().join("talk.mp4");
        if !make(
            &footage,
            &[
                "-f",
                "lavfi",
                "-i",
                "color=c=gray:s=320x180:d=4",
                "-f",
                "lavfi",
                "-i",
                "sine=f=220:d=4",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-y",
            ],
        ) {
            eprintln!("FFmpeg is unavailable; skipping the render check");
            return;
        }
        let effect = dir.path().join("whoosh.wav");
        assert!(make(
            &effect,
            &["-f", "lavfi", "-i", "sine=f=1800:d=0.3", "-y"]
        ));
        let database = dir.path().join("arrumo.db");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let root = crate::asset_root(&database);
        let footage = store.import_asset(&root, workspace, &footage).unwrap().id;
        let effect = store.import_asset(&root, workspace, &effect).unwrap().id;
        let profile = store
            .create_profile(workspace, "Talks", "", None)
            .unwrap()
            .id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Talk".into(),
                    intent: "A short talk".into(),
                    profile_id: Some(profile),
                    source_ids: vec![footage],
                },
            )
            .unwrap();
        let options = RunOptions {
            autopilot: true,
            ..RunOptions::default()
        };
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &options)
            .unwrap()
            .id;
        for (stage, result) in [
            (ProductionStage::Script, json!({"markdown": "## Hello"})),
            (
                ProductionStage::Assets,
                json!({"summary": "None", "asset_plan": []}),
            ),
        ] {
            let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
            store
                .submit_production_result(workspace, run, attempt, stage, &result)
                .unwrap();
        }
        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let delivery = crate::delivery_directory(&database, workspace, run);
        std::fs::create_dir_all(&delivery).unwrap();
        let voice = format!("voice-{attempt}-0.wav");
        assert!(make(
            &delivery.join(&voice),
            &["-f", "lavfi", "-i", "sine=f=440:d=1", "-y"]
        ));
        // The second format is cut its own way, starting 0.8 s in: the voice line still plays
        // there, and the effect, whose moment is cut, is left out.
        let formats = store.list_profile_formats(workspace, profile).unwrap();
        let own_cut = formats.last().unwrap().id;
        let edit = json!({
            "summary": "Cut",
            "cuts": [{"asset_id": footage, "start_ms": 0, "end_ms": 4000}],
            "format_cuts": {own_cut.to_string(): [{"asset_id": footage, "start_ms": 800, "end_ms": 4000}]},
            "sound_cues": [{"cut": 0, "at_ms": 500, "asset_id": effect}],
            "voice_cues": [{"cut": 0, "at_ms": 1000, "text": "Hello", "file": voice}],
            // An animation that only shows a workspace clip, drawn over the cut.
            "animations": [{
                "title": "Inset", "kind": "animation", "usage": "Picture in picture",
                "durationSeconds": 2, "background": "#102030",
                "layers": [{"type": "video", "text": null, "assetId": footage, "x": 0.75, "y": 0.25,
                    "width": 0.3, "height": 0.3, "size": 0.05, "color": "#FFFFFF", "bold": false,
                    "opacity": 1, "start": 0, "end": 2, "enter": "fade", "exit": "fade",
                    "sound": "none", "clipStart": 0, "zoom": []}]
            }],
            "motion_inserts": [],
            "motion_overlays": [{"cut": 0, "animation": 0, "at_ms": 500}],
        });
        store
            .render_preview_videos(&database, workspace, run, attempt, &edit)
            .unwrap();
        let previews: Vec<std::path::PathBuf> = std::fs::read_dir(&delivery)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("preview-") && name.ends_with(".mp4"))
            })
            .collect();
        assert!(!previews.is_empty());
        for preview in previews {
            let probe = std::process::Command::new(crate::media_tools::ffprobe())
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "a:0",
                    "-show_entries",
                    "stream=duration",
                ])
                .args(["-of", "csv=p=0"])
                .arg(&preview)
                .output()
                .unwrap();
            let seconds: f64 = String::from_utf8_lossy(&probe.stdout)
                .trim()
                .parse()
                .unwrap();
            let own = preview
                .to_string_lossy()
                .ends_with(&format!("-{own_cut}.mp4"));
            let expected = if own { 3.2 } else { 4.0 };
            assert!(
                (seconds - expected).abs() < 0.3,
                "the sound runs to the end: {seconds}"
            );
        }
        let main = store.edit_cues(&database, workspace, run, &edit).unwrap();
        assert_eq!(main.len(), 2);
        assert!(main.iter().all(|cue| cue.source.is_some()));
        // A sound effect that is not this workspace's audio is refused.
        let mut foreign = edit.clone();
        foreign["sound_cues"][0]["asset_id"] = json!(footage + 100);
        assert!(
            store
                .edit_cues(&database, workspace, run, &foreign)
                .is_err()
        );
    }

    #[test]
    fn cues_land_on_the_same_moment_in_a_format_cut_its_own_way() {
        use super::{AudioCue, PreviewCut, RenderClip, cue_position};
        let clip = |asset_id: i64, start_ms: u32, end_ms: u32| RenderClip {
            asset_id,
            path: std::path::PathBuf::new(),
            cut: Some(PreviewCut {
                asset_id,
                start_ms,
                end_ms,
                grade: None,
            }),
            has_audio: true,
        };
        // The main edit's cut 1 is footage 7 from 10 s; the line starts 1.5 s into it.
        let cue = AudioCue {
            cut: 1,
            at: 1.5,
            path: std::path::PathBuf::new(),
            voice: true,
            source: Some((7, 11_500)),
        };
        let portrait = [clip(7, 11_000, 14_000), clip(3, 0, 2_000)];
        assert_eq!(cue_position(&cue, &portrait), Some((0, 0.5)));
        let without_that_moment = [clip(7, 12_000, 14_000)];
        assert_eq!(cue_position(&cue, &without_that_moment), None);
        let unanchored = AudioCue {
            source: None,
            ..cue
        };
        assert_eq!(
            cue_position(&unanchored, &without_that_moment),
            Some((1, 1.5))
        );
    }

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
        assert_eq!(super::Grade::NEUTRAL.clamped(), None);
        let warm = super::Grade {
            brightness: 40,
            contrast: 110,
            saturation: 100,
            warmth: 5,
        }
        .clamped()
        .unwrap();
        assert_eq!(warm.brightness, 15);
        assert_eq!(
            warm.filter(),
            "eq=brightness=0.15:contrast=1.10:saturation=1.00,colorbalance=rm=0.030:bm=-0.030:rh=0.015:bh=-0.015,"
        );
        assert!(!duration_matches(10.5, &[4.0, 6.0]));
        assert!(!duration_matches(8.0, &[4.0, 6.0]));
    }
}
