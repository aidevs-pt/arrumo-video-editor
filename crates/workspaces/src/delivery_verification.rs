use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use rusqlite::{Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

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
    pub subtitles: String,
    duration_seconds: f64,
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
        let outputs = snapshot
            .formats
            .into_iter()
            .map(|entry| verify_output(&directory, entry.format))
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
             RETURNING id, draft_id, state, snapshot_json, pending_review_id",
            params![workspace_id, run_id, attempt, index],
            read_run,
        )?;
        tx.execute(
            "INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, index, attempt, result],
        )?;
        tx.commit()?;
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
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(io::Error::other(format!(
            "Missing or empty delivery file: {filename}"
        )));
    }
    let output = Command::new("ffprobe")
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
        .arg(path.canonicalize()?)
        .output()?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(io::Error::other(format!("Could not verify {filename}")));
    }
    Ok(output.stdout)
}

fn verify_output(directory: &Path, format: Format) -> Result<VerifiedOutput, Box<dyn Error>> {
    let video = format!("{}.mp4", format.id);
    let subtitles = format!("{}.srt", format.id);
    let media: VideoProbe = serde_json::from_slice(&probe(
        directory,
        &video,
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
    let captions: SubtitleProbe = serde_json::from_slice(&probe(
        directory,
        &subtitles,
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
    Ok(VerifiedOutput {
        format_id: format.id,
        video,
        subtitles,
        duration_seconds: duration,
        subtitle_cues: captions.packets.len(),
    })
}
