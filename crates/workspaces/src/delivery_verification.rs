use crate::IoStep;
use crate::Launch;
use std::{
    error::Error,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::format_deliveries::read_settings;
use crate::generation_queue::read_run;
use crate::{GenerationPlan, GenerationRun, ProductionStage, WorkspaceStore};

#[derive(Deserialize)]
struct Snapshot {
    formats: Vec<DeliveryFormat>,
}

#[derive(Deserialize)]
struct DeliveryFormat {
    format: Format,
    #[serde(default)]
    delivery: DeliveryContract,
}

/// What a format's delivery must contain, captured when the video was queued.
#[derive(Deserialize)]
struct DeliveryContract {
    required: Vec<String>,
}

impl Default for DeliveryContract {
    /// Runs captured before the contract was recorded always needed subtitles.
    fn default() -> Self {
        Self {
            required: vec!["final_video".into(), "subtitles".into()],
        }
    }
}

impl DeliveryFormat {
    fn needs_subtitles(&self) -> bool {
        self.delivery
            .required
            .iter()
            .any(|item| item == "subtitles")
    }
}

#[derive(Deserialize)]
struct Format {
    id: i64,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct VideoProbe {
    streams: Vec<Dimensions>,
    format: MediaDuration,
}

#[derive(Deserialize)]
struct Dimensions {
    width: u32,
    height: u32,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct MediaDuration {
    duration: String,
}

#[derive(Deserialize)]
struct SubtitleProbe {
    packets: Vec<Cue>,
}

#[derive(Deserialize)]
struct Cue {
    pts_time: String,
    duration_time: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct VerifiedDelivery {
    pub verified_outputs: Vec<VerifiedOutput>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct VerifiedOutput {
    pub format_id: i64,
    pub video: String,
    /// None when the video was delivered without subtitles.
    #[serde(default)]
    pub subtitles: Option<String>,
    pub(crate) duration_seconds: f64,
    #[serde(default)]
    subtitle_cues: usize,
}

pub fn delivery_directory(database: &Path, workspace_id: i64, run_id: i64) -> PathBuf {
    let mut root = database.as_os_str().to_owned();
    root.push(".deliveries");
    PathBuf::from(root)
        .join(workspace_id.to_string())
        .join(run_id.to_string())
}

impl WorkspaceStore {
    pub fn process_next_ready_delivery(&self, database: &Path) -> Result<bool, Box<dyn Error>> {
        let mut query = self.0.prepare(
            "SELECT workspace_id, id, COALESCE(json_extract(snapshot_json, '$.formats'), '[]')
             FROM generation_runs
             WHERE (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
               AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'verify_delivery'
             ORDER BY id",
        )?;
        let candidates = query
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(query);
        for (workspace_id, run_id, formats) in candidates {
            let formats: Vec<DeliveryFormat> = serde_json::from_str(&formats)?;
            if formats.is_empty() {
                continue;
            }
            if missing_delivery_files(database, workspace_id, run_id, &formats).is_empty()
                && self
                    .process_media_stage_at(
                        database,
                        workspace_id,
                        run_id,
                        ProductionStage::VerifyDelivery,
                    )?
                    .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn verified_delivery(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<VerifiedDelivery> {
        self.0.query_row(
            "SELECT results.result_json FROM production_results AS results
             JOIN generation_runs AS runs ON runs.id = results.run_id
             WHERE runs.workspace_id = ?1 AND runs.id = ?2 AND runs.state = 'completed'
               AND results.stage_index = runs.stage_index - 1
               AND json_extract(runs.plan_json, '$.steps[' || results.stage_index || '].stage')
                   = 'verify_delivery'
             ORDER BY results.id DESC LIMIT 1",
            params![workspace_id, run_id],
            |row| read_settings(row.get(0)?, 0),
        )
    }

    pub fn verify_generation_delivery(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
    ) -> Result<GenerationRun, Box<dyn Error>> {
        let (index, plan, snapshot): (i64, GenerationPlan, String) = self.0.query_row(
            "SELECT stage_index, plan_json, snapshot_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
               AND state = 'running' AND lease_until > unixepoch() AND plan_json IS NOT NULL",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, read_settings(row.get(1)?, 1)?, row.get(2)?)),
        )?;
        if plan.steps.get(index as usize).map(|step| step.stage)
            != Some(ProductionStage::VerifyDelivery)
        {
            return Err(io::Error::other("The run is not ready for delivery verification").into());
        }
        let snapshot: Snapshot = serde_json::from_str(&snapshot)?;
        if snapshot.formats.is_empty() {
            return Err(io::Error::other("No delivery formats are configured").into());
        }
        let directory = delivery_directory(database, workspace_id, run_id);
        let formats = snapshot.formats.len();
        let outputs = snapshot
            .formats
            .into_iter()
            .map(|entry| verify_output(&directory, &entry))
            .collect::<Result<Vec<_>, _>>()?;
        let result = serde_json::to_string(&VerifiedDelivery {
            verified_outputs: outputs,
        })?;

        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let completed = tx.query_row(
            "UPDATE generation_runs SET state = 'completed', stage_index = stage_index + 1,
                    lease_until = NULL, checkpoint_json = NULL
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3 AND stage_index = ?4
               AND state = 'running' AND lease_until > unixepoch()
             RETURNING id, draft_id, state, snapshot_json, pending_review_id, display_title",
            params![workspace_id, run_id, attempt, index],
            read_run,
        )?;
        tx.execute(
            "INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, index, attempt, result],
        )?;
        tx.commit()?;
        self.queue_video_completed_hooks(workspace_id, run_id);
        let snapshot = &completed.snapshot;
        self.track_usage(
            "video_generated",
            &[
                (
                    "agent",
                    snapshot["agent_settings"]["effective_provider"].clone(),
                ),
                ("autopilot", snapshot["options"]["autopilot"].clone()),
                ("formats", serde_json::json!(formats)),
            ],
        );
        Ok(completed)
    }
}

fn probe(
    directory: &Path,
    filename: &str,
    input_format: &str,
    entries: &str,
    selection: &[&str],
) -> io::Result<Vec<u8>> {
    let path = directory.join(filename);
    let metadata = match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(io::Error::other(format!(
                "Missing delivery file: {filename}"
            )));
        }
        metadata => metadata.io_step("Checking a file")?,
    };
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(io::Error::other(format!(
            "Missing or empty delivery file: {filename}"
        )));
    }
    let output = crate::quiet_command(crate::media_tools::ffprobe())
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-f",
            input_format,
            "-show_entries",
            entries,
            "-of",
            "json",
        ])
        .args(selection)
        .arg(path.canonicalize().io_step("Resolving a path")?)
        .launch_output()?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(io::Error::other(format!("Could not verify {filename}")));
    }
    Ok(output.stdout)
}

fn verify_output(
    directory: &Path,
    entry: &DeliveryFormat,
) -> Result<VerifiedOutput, Box<dyn Error>> {
    let format = &entry.format;
    let video = format!("{}.mp4", format.id);
    let duration = verified_video_duration(directory, &video, format)?;
    let srt = format!("{}.srt", format.id);
    // Subtitles that exist are always checked, such as a storyboard's own captions.
    let (subtitles, subtitle_cues) =
        if entry.needs_subtitles() || directory.join(&srt).symlink_metadata().is_ok() {
            let cues = verify_subtitle_cues(directory, &srt, duration)?;
            (Some(srt), cues)
        } else {
            (None, 0)
        };
    Ok(VerifiedOutput {
        format_id: format.id,
        video,
        subtitles,
        duration_seconds: duration,
        subtitle_cues,
    })
}

fn verified_video_duration(
    directory: &Path,
    video: &str,
    format: &Format,
) -> Result<f64, Box<dyn Error>> {
    let media: VideoProbe = serde_json::from_slice(&probe(
        directory,
        video,
        "mov",
        "stream=width,height:format=duration",
        &["-select_streams", "v:0"],
    )?)?;
    let duration: f64 = media.format.duration.parse()?;
    if !duration.is_finite()
        || duration <= 0.0
        || !media
            .streams
            .iter()
            .any(|stream| stream.width == format.width && stream.height == format.height)
    {
        return Err(
            io::Error::other(format!("Unexpected duration or dimensions in {video}")).into(),
        );
    }
    Ok(duration)
}

pub(crate) fn verify_subtitle_cues(
    directory: &Path,
    subtitles: &str,
    duration: f64,
) -> Result<usize, Box<dyn Error>> {
    let captions: SubtitleProbe = serde_json::from_slice(&probe(
        directory,
        subtitles,
        "srt",
        "packet=pts_time,duration_time",
        &["-show_packets"],
    )?)?;
    if captions.packets.is_empty() {
        return Err(io::Error::other(format!("No subtitle cues in {subtitles}")).into());
    }
    for cue in &captions.packets {
        let start: f64 = cue.pts_time.parse()?;
        let length: f64 = cue.duration_time.parse()?;
        if !start.is_finite()
            || !length.is_finite()
            || start < 0.0
            || length <= 0.0
            || start + length > duration + 0.05
        {
            return Err(io::Error::other(format!("Invalid subtitle timing in {subtitles}")).into());
        }
    }
    Ok(captions.packets.len())
}

impl WorkspaceStore {
    pub fn import_completed_video_variant(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        source: &Path,
    ) -> Result<crate::WorkspaceAsset, Box<dyn Error + Send + Sync>> {
        if source.extension().and_then(|extension| extension.to_str()) != Some("mp4")
            || !source
                .symlink_metadata()
                .io_step("Checking a file")?
                .is_file()
        {
            return Err(io::Error::other("Expected a regular MP4 variant file").into());
        }
        let digest = variant_digest(source)?;
        let database = database.canonicalize().io_step("Resolving a path")?;
        let media_root = crate::asset_root(&database);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let delivery = self.verified_delivery(workspace_id, run_id)?;
        let expected = delivery
            .verified_outputs
            .iter()
            .find(|output| output.format_id == format_id)
            .ok_or_else(|| io::Error::other("Output format is not in the verified delivery"))?;
        if expected.video != format!("{format_id}.mp4") {
            return Err(io::Error::other("Unexpected filename in verified delivery").into());
        }
        let original = managed_delivery_file(&database, workspace_id, run_id, &expected.video)?;
        let existing = tx.query_row(
            "SELECT asset.id, asset.workspace_id, asset.name, asset.kind, asset.byte_size, asset.notes
             FROM completed_media AS media JOIN workspace_assets AS asset ON asset.id = media.asset_id
             WHERE media.run_id = ?1 AND media.format_id = ?2 AND media.variant_sha256 = ?3
               AND asset.workspace_id = ?4",
            params![run_id, format_id, digest.as_slice(), workspace_id],
            crate::assets::read_asset,
        ).optional()?;
        if let Some(asset) = existing {
            let managed = crate::asset_path(&media_root, workspace_id, asset.id);
            if variant_digest(&managed)? != digest {
                return Err(
                    io::Error::other("Previously attached video variant has changed").into(),
                );
            }
            return Ok(asset);
        }
        let count: u32 = tx.query_row(
            "SELECT count(*) FROM completed_media WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        if count >= 100 {
            return Err(io::Error::other("Completed delivery attachment limit reached").into());
        }
        let asset =
            crate::assets::copy_asset_in(&tx, &media_root, workspace_id, source, false, None)?;
        crate::assets::mark_generated(&tx, workspace_id, &[asset.id])?;
        let managed = crate::asset_path(&media_root, workspace_id, asset.id);
        let attached = (|| -> Result<(), Box<dyn Error + Send + Sync>> {
            if variant_digest(&managed)? != digest {
                return Err(io::Error::other("Video variant changed during import").into());
            }
            let original = inspect_variant(&original)?;
            let variant = inspect_variant(&managed)?;
            let source_video = &original.streams[0];
            let variant_video = &variant.streams[0];
            if (source_video.width, source_video.height)
                != (variant_video.width, variant_video.height)
            {
                return Err(
                    io::Error::other("Variant dimensions differ from the verified source").into(),
                );
            }
            let duration = positive_duration(&variant.format.duration)?;
            let video_duration =
                |stream: &Dimensions| positive_duration(stream.duration.as_deref().unwrap_or(""));
            if (duration - expected.duration_seconds).abs() > 0.05
                || (video_duration(source_video)? - video_duration(variant_video)?).abs() > 0.05
            {
                return Err(
                    io::Error::other("Variant duration differs from the verified source").into(),
                );
            }
            crate::motion_artifacts::validate_motion_file(&managed, &asset.name, "audio")?;
            tx.execute(
                "INSERT INTO completed_media (run_id, format_id, asset_id, variant_sha256)
                 VALUES (?1, ?2, ?3, ?4)",
                params![run_id, format_id, asset.id, digest.as_slice()],
            )?;
            Ok(())
        })();
        if let Err(error) = attached {
            let _ = fs::remove_file(&managed);
            return Err(error);
        }
        tx.commit()?;
        Ok(asset)
    }

    pub fn verified_delivery_files(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
    ) -> Result<(PathBuf, Option<PathBuf>), Box<dyn Error>> {
        let output = self
            .verified_delivery(workspace_id, run_id)?
            .verified_outputs
            .into_iter()
            .find(|output| output.format_id == format_id)
            .ok_or_else(|| io::Error::other("Output format is not in the verified delivery"))?;
        let file = |name: String, extension: &str| -> io::Result<PathBuf> {
            if name != format!("{format_id}.{extension}") {
                return Err(io::Error::other("Unexpected filename in verified delivery"));
            }
            managed_delivery_file(database, workspace_id, run_id, &name)
        };
        Ok((
            file(output.video, "mp4")?,
            output.subtitles.map(|name| file(name, "srt")).transpose()?,
        ))
    }
}

/// The video and subtitle files a run still needs before its delivery can be verified.
fn missing_delivery_files(
    database: &Path,
    workspace_id: i64,
    run_id: i64,
    formats: &[DeliveryFormat],
) -> Vec<String> {
    formats
        .iter()
        .flat_map(|entry| {
            let id = entry.format.id;
            let mut files = vec![format!("{id}.mp4")];
            if entry.needs_subtitles() {
                files.push(format!("{id}.srt"));
            }
            files
        })
        .filter(|name| managed_delivery_file(database, workspace_id, run_id, name).is_err())
        .collect()
}

impl WorkspaceStore {
    /// The delivery files a run still needs, such as a format's subtitles, so verifying it now
    /// would fail. Empty once every format has its video and subtitles.
    pub fn delivery_files_missing(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let formats: String = self.0.query_row(
            "SELECT COALESCE(json_extract(snapshot_json, '$.formats'), '[]') FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, run_id],
            |row| row.get(0),
        )?;
        let formats: Vec<DeliveryFormat> = serde_json::from_str(&formats)?;
        Ok(missing_delivery_files(
            database,
            workspace_id,
            run_id,
            &formats,
        ))
    }
}

pub(crate) fn managed_delivery_file(
    database: &Path,
    workspace_id: i64,
    run_id: i64,
    filename: &str,
) -> io::Result<PathBuf> {
    let path = delivery_directory(
        &database.canonicalize().io_step("Resolving a path")?,
        workspace_id,
        run_id,
    )
    .join(filename);
    let metadata = path.symlink_metadata().io_step("Checking a file")?;
    if !metadata.is_file() || metadata.len() == 0 || !crate::assets::is_canonical(&path)? {
        return Err(io::Error::other("Invalid managed delivery file"));
    }
    Ok(path)
}

fn variant_digest(path: &Path) -> io::Result<[u8; 32]> {
    if !path
        .symlink_metadata()
        .io_step("Checking a file")?
        .is_file()
    {
        return Err(io::Error::other("Video variant must be a regular file"));
    }
    let mut file = fs::File::open(path).io_step("Opening a file")?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(digest.finalize().into());
        }
        digest.update(&buffer[..count]);
    }
}

fn positive_duration(value: &str) -> io::Result<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .ok_or_else(|| io::Error::other("Invalid video duration"))
}

fn inspect_variant(path: &Path) -> Result<VideoProbe, Box<dyn Error + Send + Sync>> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing video directory"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("Invalid video filename"))?;
    let probe: VideoProbe = serde_json::from_slice(&probe(
        parent,
        name,
        "mov",
        "stream=width,height,duration:format=duration",
        &["-select_streams", "V:0"],
    )?)?;
    if probe.streams.len() != 1 {
        return Err(io::Error::other("Variant does not contain a video stream").into());
    }
    Ok(probe)
}

impl WorkspaceStore {
    pub fn generate_delivery_subtitles(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
    ) -> Result<usize, Box<dyn Error>> {
        self.generate_delivery_subtitles_with(
            database,
            workspace_id,
            run_id,
            format_id,
            |order, audio| {
                self.transcribe_ordered_wav(workspace_id, order, audio, true)
                    .map(|result| result.transcript)
            },
        )
    }

    fn generate_delivery_subtitles_with(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        transcribe: impl FnOnce(Vec<String>, fs::File) -> io::Result<crate::transcription::Transcript>,
    ) -> Result<usize, Box<dyn Error>> {
        let snapshot = self.subtitle_import_snapshot(workspace_id, run_id)?;
        let order: Vec<String> = serde_json::from_value(
            serde_json::from_str::<serde_json::Value>(&snapshot)?
                ["provider_orders"]["transcription"]["effective"].clone(),
        ).map_err(|_| io::Error::other("The run has no captured transcription provider order"))?;
        let order = self.without_inactive(order)?;
        if order.is_empty() {
            return Err(io::Error::other("The run has no selected transcription provider").into());
        }
        let format = delivery_subtitle_format(&snapshot, format_id)?;
        let video_name = format!("{format_id}.mp4");
        let video = managed_delivery_file(database, workspace_id, run_id, &video_name)?;
        let directory = video
            .parent()
            .ok_or_else(|| io::Error::other("Missing delivery directory"))?;
        // Lock a separate file so the encoder can still read the video on Windows.
        let generation_lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!(".subtitles-{format_id}.lock")))?;
        match generation_lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                return Err(io::Error::other(
                    "Subtitle generation is already running for this output",
                )
                .into());
            }
            Err(fs::TryLockError::Error(error)) => return Err(error.into()),
        }
        let subtitle_path = directory.join(format!("{format_id}.srt"));
        match subtitle_path.symlink_metadata() {
            Ok(_) => return Err(io::Error::other("Delivery subtitles already exist").into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let duration = verified_video_duration(directory, &video_name, &format)?;
        let wave_limit = crate::transcription::MAX_AUDIO_BYTES;
        if duration * 32_000.0 + 4096.0 > wave_limit as f64 {
            return Err(io::Error::other(
                "The delivery audio exceeds the transcription size limit",
            )
            .into());
        }
        let job = tempfile::tempdir()?;
        let wave = job.path().join("delivery.wav");
        let status = crate::quiet_command(crate::media_tools::ffmpeg())
            .args([
                "-nostdin",
                "-v",
                "error",
                "-protocol_whitelist",
                "file",
                "-f",
                "mov",
                "-i",
            ])
            .arg(&video)
            .args([
                "-map",
                "0:a:0",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                "-fs",
            ])
            .arg(wave_limit.to_string())
            .arg(&wave)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .launch_status()?;
        if !status.success() || wave.metadata().io_step("Checking a file")?.len() >= wave_limit {
            return Err(io::Error::other("Could not extract bounded delivery audio").into());
        }
        let transcript = transcribe(order, fs::File::open(wave).io_step("Opening a file")?)?;
        let subtitles = crate::delivery_captions::render_srt(&transcript.words, duration)?;
        self.publish_delivery_subtitles(
            [workspace_id, run_id, format_id],
            directory,
            duration,
            &subtitles,
            false,
        )
    }

    pub fn import_delivery_subtitles(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        source: &Path,
    ) -> Result<usize, Box<dyn Error>> {
        let snapshot = self.subtitle_import_snapshot(workspace_id, run_id)?;
        let format = delivery_subtitle_format(&snapshot, format_id)?;
        let video = format!("{format_id}.mp4");
        let video_path = managed_delivery_file(database, workspace_id, run_id, &video)?;
        let directory = video_path
            .parent()
            .ok_or_else(|| io::Error::other("Missing delivery directory"))?;
        let duration = verified_video_duration(directory, &video, &format)?;

        if !source.metadata().io_step("Checking a file")?.is_file() {
            return Err(io::Error::other("Choose an SRT file").into());
        }
        let subtitles = crate::read_subtitle_file(source)?;
        self.publish_delivery_subtitles(
            [workspace_id, run_id, format_id],
            directory,
            duration,
            &subtitles,
            true,
        )
    }

    fn publish_delivery_subtitles(
        &self,
        [workspace_id, run_id, format_id]: [i64; 3],
        directory: &Path,
        duration: f64,
        subtitles: &str,
        replace: bool,
    ) -> Result<usize, Box<dyn Error>> {
        use std::io::Write;

        let mut incoming = tempfile::NamedTempFile::new_in(directory)?;
        incoming.write_all(subtitles.as_bytes())?;
        let name = incoming
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("Unable to stage the subtitles"))?;
        let cues = verify_subtitle_cues(directory, name, duration)?;
        incoming.as_file().sync_all()?;
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        self.subtitle_import_snapshot(workspace_id, run_id)?;
        let destination = directory.join(format!("{format_id}.srt"));
        if replace {
            incoming.persist(destination)?;
        } else {
            incoming.persist_noclobber(destination)?;
        }
        tx.commit()?;
        Ok(cues)
    }

    fn subtitle_import_snapshot(&self, workspace_id: i64, run_id: i64) -> rusqlite::Result<String> {
        self.0.query_row(
            "SELECT snapshot_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND state IN ('queued', 'failed')
               AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'verify_delivery'",
            params![workspace_id, run_id],
            |row| row.get(0),
        )
    }
}

fn delivery_subtitle_format(snapshot: &str, format_id: i64) -> Result<Format, Box<dyn Error>> {
    serde_json::from_str::<Snapshot>(snapshot)?
        .formats
        .into_iter()
        .map(|entry| entry.format)
        .find(|format| format.id == format_id)
        .ok_or_else(|| io::Error::other("Format is not in the run snapshot").into())
}

#[cfg(all(test, target_os = "linux"))]
mod transcription_tests {
    use super::*;
    use crate::{
        DraftSettings, RunOptions,
        transcription::{Transcript, TranscriptWord},
    };
    use serde_json::json;

    #[test]
    fn generated_subtitles_complete_a_delivery_and_preserve_existing_files() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspace.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Test delivery".into(),
                    intent: "Test caption generation".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "caption-fixture",
                &RunOptions::default(),
            )
            .unwrap();
        let format_id = 5;
        let snapshot = json!({
            "formats": [{"format": {"id": format_id, "width": 160, "height": 90}}],
            "provider_orders": {"transcription": {"effective": ["fixture-timed", "fixture-fallback"]}}
        });
        let plan = json!({"steps": [{"stage": "verify_delivery", "review_required": false}]});
        store
            .0
            .execute(
                "UPDATE generation_runs SET snapshot_json = ?1, plan_json = ?2 WHERE id = ?3",
                params![snapshot.to_string(), plan.to_string(), run.id],
            )
            .unwrap();

        let directory = delivery_directory(&database, workspace, run.id);
        fs::create_dir_all(&directory).unwrap();
        let video = directory.join("5.mp4");
        let status = crate::quiet_command(crate::media_tools::ffmpeg())
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=s=160x90:r=15",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000",
                "-t",
                "1",
                "-c:v",
                "mpeg4",
                "-threads",
                "1",
                "-c:a",
                "aac",
            ])
            .arg(video)
            .launch_status()
            .unwrap();
        assert!(status.success());

        assert!(!store.process_next_automatic_subtitles(&database).unwrap());
        store.0.execute(
            "UPDATE generation_runs SET snapshot_json = json_set(snapshot_json, '$.options.generate_delivery_subtitles', json('true')) WHERE id = ?1",
            [run.id],
        ).unwrap();
        assert!(store.process_next_automatic_subtitles_with(&database, |claimed_workspace, claimed_run, claimed_format| {
            assert_eq!((claimed_workspace, claimed_run, claimed_format), (workspace, run.id, format_id));
            let state: String = store.0.query_row(
                "SELECT state FROM automatic_delivery_subtitles WHERE run_id = ?1 AND format_id = ?2",
                params![run.id, format_id],
                |row| row.get(0),
            ).unwrap();
            assert_eq!(state, "started");
            store.generate_delivery_subtitles_with(
                &database,
                workspace,
                run.id,
                format_id,
                |order, mut audio| {
                    assert_eq!(order, ["fixture-timed", "fixture-fallback"]);
                    let mut header = [0; 12];
                    audio.read_exact(&mut header)?;
                    assert_eq!(&header[0..4], b"RIFF");
                    assert_eq!(&header[8..12], b"WAVE");
                    assert!(audio.metadata().io_step("Checking a file")?.len() > 30_000);
                    Ok(Transcript {
                        language_code: "en".into(),
                        text: "Fixture audio".into(),
                        words: vec![TranscriptWord {
                            text: "Fixture audio".into(),
                            start: 0.125,
                            end: 0.75,
                            speaker_id: None,
                        }],
                    })
                },
            )
        }).unwrap());

        let srt = directory.join("5.srt");
        let expected = "1\n00:00:00,125 --> 00:00:00,750\nFixture audio\n\n";
        assert_eq!(fs::read_to_string(&srt).unwrap(), expected);
        assert!(
            store
                .generate_delivery_subtitles_with(
                    &database,
                    workspace,
                    run.id,
                    format_id,
                    |_, _| panic!("Existing subtitles must prevent provider dispatch"),
                )
                .is_err()
        );
        assert_eq!(fs::read_to_string(&srt).unwrap(), expected);
        fs::remove_file(&srt).unwrap();
        store
            .0
            .execute(
                "UPDATE automatic_delivery_subtitles SET state = 'started' WHERE run_id = ?1",
                [run.id],
            )
            .unwrap();
        drop(store);
        let store = WorkspaceStore::open(&database).unwrap();
        assert!(
            !store
                .process_next_automatic_subtitles_with(&database, |_, _, _| {
                    panic!("Restart must not resend an interrupted provider request")
                })
                .unwrap()
        );
        fs::write(&srt, expected).unwrap();
        assert!(store.process_next_ready_delivery(&database).unwrap());
        let verified = store.verified_delivery(workspace, run.id).unwrap();
        assert_eq!(verified.verified_outputs[0].subtitle_cues, 1);
        assert_eq!(
            verified.verified_outputs[0].subtitles.as_deref(),
            Some("5.srt")
        );
    }

    #[test]
    fn a_video_queued_without_subtitles_is_delivered_without_them() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspace.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "No subtitles".into(),
                    intent: "A short".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap();
        let snapshot = json!({"formats": [{
            "format": {"id": 5, "width": 160, "height": 90},
            "delivery": {"required": ["final_video"]}
        }]});
        let plan = json!({"steps": [{"stage": "verify_delivery", "review_required": false}]});
        store
            .0
            .execute(
                "UPDATE generation_runs SET snapshot_json = ?1, plan_json = ?2 WHERE id = ?3",
                params![snapshot.to_string(), plan.to_string(), run.id],
            )
            .unwrap();
        let directory = delivery_directory(&database, workspace, run.id);
        fs::create_dir_all(&directory).unwrap();
        let status = crate::quiet_command(crate::media_tools::ffmpeg())
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=s=160x90:r=15",
                "-t",
                "1",
                "-c:v",
                "mpeg4",
                "-threads",
                "1",
            ])
            .arg(directory.join("5.mp4"))
            .launch_status()
            .unwrap();
        assert!(status.success());

        assert!(store.process_next_ready_delivery(&database).unwrap());
        let output = &store
            .verified_delivery(workspace, run.id)
            .unwrap()
            .verified_outputs[0];
        assert_eq!(
            (output.subtitles.as_deref(), output.subtitle_cues),
            (None, 0)
        );
        assert_eq!(
            store
                .completed_subtitle_track(&database, workspace, run.id, 5, None, None)
                .unwrap()
                .subtitles,
            ""
        );
        let zip = temp.path().join("delivery.zip");
        store
            .export_generation_delivery(&database, workspace, run.id, &zip)
            .unwrap();
        let archive = zip::ZipArchive::new(fs::File::open(zip).unwrap()).unwrap();
        let names: Vec<String> = archive
            .file_names()
            .map(|name| name.unwrap().to_string())
            .collect();
        assert_eq!(names, ["5.mp4"]);
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use crate::{DraftSettings, RunOptions};
    use serde_json::json;

    #[test]
    fn a_delivery_names_the_files_it_still_needs() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("workspace.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Portrait".into(),
                    intent: "A short".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap();
        let snapshot = json!({"formats": [{"format": {"id": 2, "width": 1080, "height": 1920}}]});
        store
            .0
            .execute(
                "UPDATE generation_runs SET snapshot_json = ?1 WHERE id = ?2",
                params![snapshot.to_string(), run.id],
            )
            .unwrap();
        let directory = delivery_directory(&database, workspace, run.id);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("2.mp4"), b"video").unwrap();
        assert_eq!(
            store
                .delivery_files_missing(&database, workspace, run.id)
                .unwrap(),
            ["2.srt"]
        );
        fs::write(directory.join("2.srt"), b"1\n").unwrap();
        assert!(
            store
                .delivery_files_missing(&database, workspace, run.id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn queueing_without_automatic_subtitles_drops_them_from_the_delivery() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Portrait".into(),
                    intent: "A short".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let required = |key: &str, generate: bool| {
            let options = RunOptions {
                generate_delivery_subtitles: generate,
                ..RunOptions::default()
            };
            let run = store
                .enqueue_video_draft(workspace, draft.id, key, &options)
                .unwrap();
            run.snapshot["formats"][0]["delivery"]["required"].clone()
        };
        assert_eq!(required("with", true), json!(["final_video", "subtitles"]));
        assert_eq!(required("without", false), json!(["final_video"]));
    }

    #[test]
    fn a_missing_file_is_named_when_checked() {
        let temp = tempfile::tempdir().unwrap();
        let error = probe(temp.path(), "2.srt", "srt", "packet=pts", &[]).unwrap_err();
        assert_eq!(error.to_string(), "Missing delivery file: 2.srt");
    }
}
