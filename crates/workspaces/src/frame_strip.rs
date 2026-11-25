//! A picture of a stretch of video for agents and reviewers: evenly spaced frames above the
//! sound's waveform, to check a cut, a join or an overlay without watching the whole video.

use crate::IoStep;
use crate::Launch;
use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    process::Stdio,
};

use serde::Serialize;

use crate::WorkspaceStore;

const TILE: u32 = 240;
const WAVE_HEIGHT: u32 = 160;
const MAX_SPAN: f64 = 120.0;

/// Which video to look at.
#[derive(Debug, Clone, Copy)]
pub enum FrameStripSource {
    Asset(i64),
    /// A run's final video for a format, or its preview from that attempt.
    Output {
        run_id: i64,
        format_id: i64,
        preview_attempt: Option<i64>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStrip {
    pub destination: String,
    /// The time of each frame, left to right, in seconds.
    pub frame_times: Vec<f64>,
    pub has_audio: bool,
}

fn probe(video: &Path) -> Result<(f64, bool, bool), Box<dyn Error>> {
    let output = crate::quiet_command(crate::media_tools::ffprobe())
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type",
            "-of",
            "json",
        ])
        .arg(video)
        .stdin(Stdio::null())
        .launch_output()?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let duration = value["format"]["duration"]
        .as_str()
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| io::Error::other("Could not read the video's length"))?;
    let has = |kind: &str| {
        value["streams"]
            .as_array()
            .is_some_and(|streams| streams.iter().any(|stream| stream["codec_type"] == kind))
    };
    Ok((duration, has("video"), has("audio")))
}

/// Draws `frames` frames between `start` and `end` seconds above the waveform into `output`,
/// returning each frame's time. Callers check the range.
fn draw_strip(
    video: &Path,
    start: f64,
    end: f64,
    frames: u32,
    has_audio: bool,
    output: &Path,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let span = end - start;
    let frame_times: Vec<f64> = (0..frames)
        .map(|index| {
            ((start + span * (f64::from(index) + 0.5) / f64::from(frames)) * 1000.0).round()
                / 1000.0
        })
        .collect();
    let width = TILE * frames;
    let strip = format!(
        "[0:v]fps={frames}/{span:.6},scale={TILE}:{TILE}:force_original_aspect_ratio=decrease,\
         pad={TILE}:{TILE}:(ow-iw)/2:(oh-ih)/2:color=0x111111,tile={frames}x1[strip]"
    );
    let graph = if has_audio {
        format!(
            "{strip};[0:a]aformat=channel_layouts=mono,showwavespic=s={width}x{WAVE_HEIGHT}:colors=0x6fb3d2[wave];\
             color=c=0x111111:s={width}x{WAVE_HEIGHT}:d=1[panel];[panel][wave]overlay=format=auto,format=rgb24[back];\
             [strip]format=rgb24[frames];[frames][back]vstack[out]"
        )
    } else {
        format!("{strip};[strip]null[out]")
    };
    let status = crate::quiet_command(crate::media_tools::ffmpeg())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-ss",
            &format!("{start:.3}"),
            "-t",
            &format!("{span:.3}"),
            "-i",
        ])
        .arg(video)
        .args([
            "-filter_complex",
            &graph,
            "-map",
            "[out]",
            "-frames:v",
            "1",
            "-y",
        ])
        .arg(output)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .launch_status()?;
    if !status.success() {
        return Err(io::Error::other("Could not draw the frame strip").into());
    }
    Ok(frame_times)
}

/// Frame size in a clip overview: larger than a strip's, so the agent can judge colour.
const OVERVIEW_TILE: u32 = 320;

/// A whole clip at a glance for an agent choosing cuts: `frames` frames across its full length
/// above its waveform, written to `output`. Each frame is read with its own seek and the
/// waveform from the sound alone, so a long recording is never decoded in full. Returns each
/// frame's time.
pub(crate) fn clip_overview(
    video: &Path,
    frames: u32,
    output: &Path,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (duration, has_video, has_audio) = probe(video)?;
    if !has_video || duration < 0.1 || frames == 0 {
        return Err(io::Error::other("The clip has no picture to show").into());
    }
    let times: Vec<f64> = (0..frames)
        .map(|index| {
            (duration * (f64::from(index) + 0.5) / f64::from(frames) * 10.0).round() / 10.0
        })
        .collect();
    let work = tempfile::tempdir()?;
    let tile = format!(
        "scale={OVERVIEW_TILE}:{OVERVIEW_TILE}:force_original_aspect_ratio=decrease,\
         pad={OVERVIEW_TILE}:{OVERVIEW_TILE}:(ow-iw)/2:(oh-ih)/2:color=0x111111,format=rgb24"
    );
    for (index, at) in times.iter().enumerate() {
        let status = crate::quiet_command(crate::media_tools::ffmpeg())
            .args(["-nostdin", "-v", "error", "-ss", &format!("{at:.3}"), "-i"])
            .arg(video)
            .args(["-frames:v", "1", "-vf", &tile, "-y"])
            .arg(work.path().join(format!("{index}.png")))
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .launch_status()?;
        if !status.success() {
            return Err(io::Error::other("Could not read a frame of the clip").into());
        }
    }
    let width = OVERVIEW_TILE * frames;
    let mut command = crate::quiet_command(crate::media_tools::ffmpeg());
    command.args(["-nostdin", "-v", "error"]);
    for index in 0..frames {
        command
            .arg("-i")
            .arg(work.path().join(format!("{index}.png")));
    }
    let inputs: String = (0..frames).map(|index| format!("[{index}:v]")).collect();
    let row = if frames == 1 {
        format!("{inputs}null[frames]")
    } else {
        format!("{inputs}hstack=inputs={frames}[frames]")
    };
    let graph = if has_audio {
        command.arg("-i").arg(video);
        format!(
            "{row};[{frames}:a]aformat=channel_layouts=mono,showwavespic=s={width}x{WAVE_HEIGHT}:colors=0x6fb3d2:scale=sqrt[wave];\
             color=c=0x111111:s={width}x{WAVE_HEIGHT}:d=1[panel];[panel][wave]overlay=format=auto,format=rgb24[back];\
             [frames][back]vstack[out]"
        )
    } else {
        format!("{row};[frames]null[out]")
    };
    let status = command
        .args([
            "-filter_complex",
            &graph,
            "-map",
            "[out]",
            "-frames:v",
            "1",
            "-y",
        ])
        .arg(output)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .launch_status()?;
    if !status.success() {
        return Err(io::Error::other("Could not draw the clip overview").into());
    }
    Ok(times)
}

/// Draws `frames` frames between `start` and `end` seconds above the waveform into a new PNG.
pub fn render_frame_strip(
    video: &Path,
    start: f64,
    end: f64,
    frames: u32,
    destination: &Path,
) -> Result<FrameStrip, Box<dyn Error>> {
    if !destination.is_absolute()
        || destination
            .extension()
            .is_none_or(|extension| extension != "png")
    {
        return Err(io::Error::other("Choose an absolute path ending in .png").into());
    }
    if destination.symlink_metadata().is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "The destination already exists",
        )
        .into());
    }
    let (duration, has_video, has_audio) = probe(video)?;
    let end = end.min(duration);
    if !has_video
        || !(2..=16).contains(&frames)
        || !start.is_finite()
        || start < 0.0
        || end - start < 0.1
        || end - start > MAX_SPAN
    {
        return Err(io::Error::other(format!(
            "Choose 2 to 16 frames and a range of 0.1 to {MAX_SPAN} seconds inside the video's {duration:.2} seconds"
        ))
        .into());
    }
    let work = tempfile::tempdir()?;
    let output: PathBuf = work.path().join("strip.png");
    let frame_times = draw_strip(video, start, end, frames, has_audio, &output)?;
    let png = std::fs::read(&output).io_step("Reading a file")?;
    std::io::Write::write_all(
        &mut std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?,
        &png,
    )?;
    Ok(FrameStrip {
        destination: destination.to_string_lossy().into_owned(),
        frame_times,
        has_audio,
    })
}

impl WorkspaceStore {
    /// The local file behind a frame strip source, checked to belong to the workspace.
    pub fn frame_strip_source(
        &self,
        database: &Path,
        workspace_id: i64,
        source: FrameStripSource,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let path = match source {
            FrameStripSource::Asset(asset_id) => {
                let kind: String = self.0.query_row(
                    "SELECT kind FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2",
                    rusqlite::params![workspace_id, asset_id],
                    |row| row.get(0),
                )?;
                if kind != "video" {
                    return Err(io::Error::other("That asset is not a video").into());
                }
                crate::asset_path(&crate::asset_root(database), workspace_id, asset_id)
            }
            FrameStripSource::Output {
                run_id,
                format_id,
                preview_attempt,
            } => {
                let owned: bool = self.0.query_row(
                    "SELECT EXISTS(SELECT 1 FROM generation_runs WHERE workspace_id = ?1 AND id = ?2)",
                    rusqlite::params![workspace_id, run_id],
                    |row| row.get(0),
                )?;
                if !owned || format_id <= 0 || preview_attempt.is_some_and(|attempt| attempt <= 0) {
                    return Err(io::Error::other("No such video").into());
                }
                crate::delivery_directory(database, workspace_id, run_id).join(
                    match preview_attempt {
                        Some(attempt) => format!("preview-{attempt}-{format_id}.mp4"),
                        None => format!("{format_id}.mp4"),
                    },
                )
            }
        };
        if !path.is_file() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "That video has no file yet").into(),
            );
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overviews_cover_a_whole_clip_longer_than_a_strip() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("long.mp4");
        let made = std::process::Command::new(crate::media_tools::ffmpeg())
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=160x90:d=130:r=5",
                "-f",
                "lavfi",
                "-i",
                "sine=f=300:d=130",
                "-shortest",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-y",
            ])
            .arg(&video)
            .launch_status()
            .is_ok_and(|status| status.success());
        if !made {
            eprintln!("FFmpeg is unavailable; skipping the overview check");
            return;
        }
        let image = dir.path().join("overview.png");
        let times = super::clip_overview(&video, 6, &image).unwrap();
        assert_eq!(times.len(), 6);
        assert!(times[5] > 110.0, "the frames reach the end: {times:?}");
        assert!(image.is_file());
    }

    #[test]
    fn draws_frames_above_the_waveform() {
        if crate::quiet_command(crate::media_tools::ffmpeg())
            .arg("-version")
            .launch_output()
            .is_err()
        {
            return;
        }
        let work = tempfile::tempdir().unwrap();
        let video = work.path().join("clip.mp4");
        let status = crate::quiet_command(crate::media_tools::ffmpeg())
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=s=320x180:d=3",
                "-f",
                "lavfi",
                "-i",
                "sine=d=3",
                "-shortest",
                "-y",
            ])
            .arg(&video)
            .launch_status()
            .unwrap();
        assert!(status.success());
        let destination = work.path().join("view.png");
        let view = render_frame_strip(&video, 1.0, 3.0, 4, &destination).unwrap();
        assert!(view.has_audio);
        assert_eq!(view.frame_times, vec![1.25, 1.75, 2.25, 2.75]);
        let png = std::fs::read(&destination).unwrap();
        let size = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
        assert_eq!((size(16), size(20)), (4 * TILE, TILE + WAVE_HEIGHT));
        assert!(
            render_frame_strip(&video, 0.0, 1.0, 4, &destination).is_err(),
            "never overwrites"
        );
        assert!(render_frame_strip(&video, 2.0, 1.0, 4, &work.path().join("bad.png")).is_err());
    }
}
