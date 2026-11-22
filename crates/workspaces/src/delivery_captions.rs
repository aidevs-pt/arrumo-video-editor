use std::{fmt::Write, io};

use crate::transcription::TranscriptWord;

fn timestamp(millis: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        millis / 3_600_000,
        millis / 60_000 % 60,
        millis / 1000 % 60,
        millis % 1000,
    )
}

/// libass styles SRT on a 288-line canvas scaled to the video's height. Text is about 5.5% of
/// the shorter side; portrait captions sit higher, above the controls short-video apps overlay.
pub(crate) fn burned_caption_style(width: u32, height: u32) -> String {
    let canvas = |share: f64| share * 288.0;
    let size = canvas(0.055) * f64::from(width.min(height)) / f64::from(height);
    let margin = canvas(if height > width { 0.2 } else { 0.07 });
    format!(
        "FontName=Sans,Bold=1,FontSize={size:.1},PrimaryColour=&H00FFFFFF,OutlineColour=&H00000000,BorderStyle=1,Outline={:.1},Shadow=0,Alignment=2,MarginV={margin:.0}",
        size * 0.1
    )
}

/// Draws `subtitles` into a copy of `video` at `output`, keeping its audio as it is.
pub(crate) fn burn_captions(
    video: &std::path::Path,
    subtitles: &str,
    (width, height): (u32, u32),
    output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = output
        .parent()
        .ok_or_else(|| io::Error::other("The captioned copy needs a directory"))?;
    std::fs::write(directory.join("captions.srt"), subtitles)?;
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-i",
        ])
        .arg(video)
        .arg("-vf")
        .arg(format!(
            "subtitles=filename=captions.srt:charenc=UTF-8:force_style='{}'",
            burned_caption_style(width, height)
        ))
        .args([
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p",
        ])
        .args(["-c:a", "copy", "-movflags", "+faststart", "-y"])
        .arg(output)
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(io::Error::other("Could not draw the subtitles into the video").into());
    }
    Ok(())
}

pub(crate) fn render_srt(words: &[TranscriptWord], duration: f64) -> io::Result<String> {
    let mut output = String::new();
    let mut cue_number = 0;
    let mut cue_start = 0;
    let mut cue_end = 0;
    let mut previous_start = 0.0;
    let mut text = String::new();
    let mut flush = |start, end, text: &str| {
        cue_number += 1;
        let _ = writeln!(
            output,
            "{cue_number}\n{} --> {}\n{text}\n",
            timestamp(start),
            timestamp(end)
        );
    };
    for word in words {
        if !word.start.is_finite()
            || !word.end.is_finite()
            || word.start < previous_start
            || word.end < word.start
            || word.end > duration + 0.05
        {
            return Err(io::Error::other(
                "Transcription returned invalid caption timing",
            ));
        }
        previous_start = word.start;
        let start = (word.start * 1000.0).round() as u64;
        let end = (word.end.min(duration) * 1000.0).round() as u64;
        let word = word.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if word.is_empty() || end <= start {
            continue;
        }
        if !text.is_empty()
            && (start > cue_end + 750
                || end > cue_start + 5000
                || text.chars().count() + word.chars().count() + 1 > 60)
        {
            flush(cue_start, cue_end, &text);
            text.clear();
        }
        if text.is_empty() {
            cue_start = start;
            cue_end = end;
        } else {
            if !matches!(word.as_str(), "." | "," | "!" | "?" | ":" | ";") {
                text.push(' ');
            }
            cue_end = cue_end.max(end);
        }
        text.push_str(&word);
    }
    if !text.is_empty() {
        flush(cue_start, cue_end, &text);
    }
    if output.is_empty() {
        return Err(io::Error::other(
            "Transcription did not return timed caption text",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn caption_size_follows_the_shorter_side() {
        let landscape = super::burned_caption_style(1920, 1080);
        let portrait = super::burned_caption_style(1080, 1920);
        assert!(landscape.contains("FontSize=15.8,") && landscape.contains("MarginV=20"));
        assert!(portrait.contains("FontSize=8.9,") && portrait.contains("MarginV=58"));
    }

    use super::*;

    #[test]
    fn renders_real_timestamps_and_rejects_missing_or_invalid_timings() {
        let word = |text: &str, start, end| TranscriptWord {
            text: text.into(),
            start,
            end,
            speaker_id: None,
        };
        let words = [
            word("First", 0.125, 0.48),
            word("line", 0.50, 0.8),
            word("Next\nline", 2.0, 2.401),
        ];
        assert_eq!(
            render_srt(&words, 2.4).unwrap(),
            "1\n00:00:00,125 --> 00:00:00,800\nFirst line\n\n2\n00:00:02,000 --> 00:00:02,400\nNext line\n\n"
        );
        assert!(render_srt(&[], 2.4).is_err());
        assert!(render_srt(&words, 1.0).is_err());
    }
}
