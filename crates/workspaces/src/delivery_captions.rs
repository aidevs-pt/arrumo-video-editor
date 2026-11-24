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

/// Draws `subtitles` into a copy of `video` at `output`, keeping its audio as it is.
pub(crate) fn burn_captions(
    video: &std::path::Path,
    subtitles: &str,
    (width, height): (u32, u32),
    style: &crate::CaptionStyle,
    output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    style.check()?;
    let directory = output
        .parent()
        .ok_or_else(|| io::Error::other("The captioned copy needs a directory"))?;
    std::fs::write(directory.join("captions.srt"), subtitles)?;
    let status = std::process::Command::new(crate::media_tools::ffmpeg())
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
            style.force_style(width, height)
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
    let mut speaker: Option<&str> = None;
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
        let word_speaker = word.speaker_id.as_deref();
        let word = word.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if word.is_empty() || end <= start {
            continue;
        }
        let changed = !text.is_empty()
            && speaker.is_some()
            && word_speaker.is_some()
            && word_speaker != speaker;
        if !text.is_empty()
            && (changed
                || start > cue_end + 750
                || end > cue_start + 5000
                || text.chars().count() + word.chars().count() + 1 > 60)
        {
            flush(cue_start, cue_end, &text);
            text.clear();
        }
        speaker = word_speaker.or(speaker);
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
        let spoken = |text: &str, start, end, speaker: &str| TranscriptWord {
            text: text.into(),
            start,
            end,
            speaker_id: Some(speaker.into()),
        };
        let dialogue = [
            spoken("Hi", 0.0, 0.3, "a"),
            spoken("there", 0.3, 0.6, "a"),
            spoken("Hello", 0.7, 1.0, "b"),
        ];
        assert_eq!(
            render_srt(&dialogue, 1.0).unwrap(),
            "1\n00:00:00,000 --> 00:00:00,600\nHi there\n\n2\n00:00:00,700 --> 00:00:01,000\nHello\n\n"
        );
        assert!(render_srt(&words, 1.0).is_err());
    }
}
