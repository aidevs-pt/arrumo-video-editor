//! Transcripts of source footage. Audio is extracted in bounded chunks, sent to the run's
//! transcription providers in their captured order, and cached per managed asset.

use std::{
    error::Error,
    fs, io,
    path::Path,
    process::{Command, Stdio},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{TranscriptWord, WorkspaceStore, asset_path, asset_root};

/// 15 minutes of 16 kHz mono PCM stays below the transcription upload limit.
const CHUNK_SECONDS: f64 = 900.0;
const MAX_SOURCE_SECONDS: f64 = 4.0 * 60.0 * 60.0;
/// A pause this long or longer ends a transcript line; such pauses are the cleanest cut points.
const PHRASE_PAUSE_SECONDS: f64 = 0.5;

#[derive(Debug, Serialize)]
pub struct SourceTranscript {
    pub provider: String,
    pub language_code: String,
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

impl SourceTranscript {
    /// Phrase lines with their start and end in seconds, as `[012.40-015.85] text`. A line ends at
    /// a sentence, before a pause of half a second or more, at a change of speaker, or after 24
    /// words, so pauses (the cleanest cut points) show as gaps between ranges. Speakers are named
    /// (`S0`, `S1`) only when there is more than one.
    pub fn timed_lines(&self, max_chars: usize) -> String {
        if self.words.is_empty() {
            return self.text.chars().take(max_chars).collect();
        }
        let speakers: std::collections::BTreeSet<&str> = self
            .words
            .iter()
            .filter_map(|word| word.speaker_id.as_deref())
            .collect();
        let named = speakers.len() > 1;
        let mut lines = String::new();
        let mut current = String::new();
        let mut range = (0.0, 0.0);
        let mut speaker: Option<&str> = None;
        let mut count = 0;
        let flush =
            |lines: &mut String, current: &mut String, range: (f64, f64), speaker: Option<&str>| {
                if current.is_empty() {
                    return;
                }
                let who = match (named, speaker) {
                    (true, Some(id)) => format!(
                        " S{}",
                        speakers.iter().position(|known| *known == id).unwrap_or(0)
                    ),
                    _ => String::new(),
                };
                lines.push_str(&format!(
                    "[{:06.2}-{:06.2}]{who} {current}\n",
                    range.0, range.1
                ));
                current.clear();
            };
        for word in &self.words {
            let text = word.text.trim();
            if text.is_empty() {
                continue;
            }
            let paused = !current.is_empty() && word.start - range.1 >= PHRASE_PAUSE_SECONDS;
            let changed = !current.is_empty() && named && word.speaker_id.as_deref() != speaker;
            if paused || changed {
                flush(&mut lines, &mut current, range, speaker);
                count = 0;
            }
            if current.is_empty() {
                range.0 = word.start;
                speaker = word.speaker_id.as_deref();
            } else if !text.starts_with(|c: char| c.is_ascii_punctuation()) {
                current.push(' ');
            }
            current.push_str(text);
            range.1 = word.end.max(word.start);
            count += 1;
            if current.ends_with(['.', '?', '!']) || count >= 24 {
                flush(&mut lines, &mut current, range, speaker);
                count = 0;
            }
            if lines.len() > max_chars {
                break;
            }
        }
        flush(&mut lines, &mut current, range, speaker);
        lines.chars().take(max_chars).collect()
    }

    /// What `timed_lines(inline)` leaves out, as whole lines grouped into parts of at most
    /// `part` characters, each with the time of its first and last line.
    pub(crate) fn remaining_parts(&self, inline: usize, part: usize) -> Vec<TranscriptPart> {
        let full = self.timed_lines(usize::MAX);
        let mut shown = 0;
        let mut parts: Vec<TranscriptPart> = Vec::new();
        for line in full.lines() {
            let length = line.chars().count() + 1;
            if shown + length <= inline {
                shown += length;
                continue;
            }
            shown = inline;
            let (from, to) = line
                .strip_prefix('[')
                .and_then(|rest| rest.split_once(']'))
                .and_then(|(range, _)| range.split_once('-'))
                .map(|(from, to)| (from.to_owned(), to.to_owned()))
                .unwrap_or_default();
            match parts.last_mut() {
                Some(current) if current.text.chars().count() + length <= part => {
                    current.text.push_str(line);
                    current.text.push('\n');
                    current.to = to;
                }
                _ => parts.push(TranscriptPart {
                    text: format!("{line}\n").chars().take(part).collect(),
                    from,
                    to,
                }),
            }
        }
        parts
    }
}

/// Padding kept before the first spoken word of a cut; transcript timings drift by 50–100 ms.
const CUT_LEAD_SECONDS: f64 = 0.05;
/// Padding kept after the last spoken word of a cut.
const CUT_TAIL_SECONDS: f64 = 0.08;

/// Moves a cut's edges so it never splits a word. An edge inside a word moves to the nearer
/// side: the cut keeps the word when the edge covers most of it and drops it otherwise. A kept
/// first or last word gets padding, but never reaching into the neighbouring word. Edges in a
/// longer silence stay where they are.
pub(crate) fn snap_cut(words: &[TranscriptWord], start: f64, end: f64) -> (f64, f64) {
    let lead = |index: usize| {
        let word = &words[index];
        let previous = words[..index]
            .iter()
            .map(|word| word.end)
            .fold(0.0_f64, f64::max)
            .min(word.start);
        (word.start - CUT_LEAD_SECONDS).max(previous).max(0.0)
    };
    let tail = |index: usize| {
        let word = &words[index];
        let next = words[index + 1..]
            .iter()
            .map(|word| word.start)
            .filter(|next| *next >= word.end)
            .fold(f64::INFINITY, f64::min);
        (word.end + CUT_TAIL_SECONDS).min(next)
    };
    let mut snapped = (start, end);
    if let Some(index) = words.iter().position(|word| word.end > start) {
        let word = &words[index];
        if start > word.start && start - word.start > word.end - start {
            snapped.0 = match words.get(index + 1) {
                Some(next) if next.start < end => lead(index + 1).max(word.end),
                _ => word.end,
            };
        } else if word.start - start < CUT_LEAD_SECONDS && word.start < end {
            snapped.0 = lead(index);
        }
    }
    if let Some(index) = words.iter().rposition(|word| word.start < end) {
        let word = &words[index];
        if end < word.end && word.end - end > end - word.start {
            snapped.1 = match index.checked_sub(1).map(|before| (before, &words[before])) {
                Some((before, kept)) if kept.end > start => tail(before).min(word.start),
                _ => word.start,
            };
        } else if end - word.end < CUT_TAIL_SECONDS && word.end > snapped.0 {
            snapped.1 = tail(index);
        }
    }
    if snapped.1 > snapped.0 {
        snapped
    } else {
        (start, end)
    }
}

/// A stretch of a transcript beyond what a prompt includes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptPart {
    pub text: String,
    pub from: String,
    pub to: String,
}

impl WorkspaceStore {
    pub fn source_transcript(
        &self,
        workspace_id: i64,
        asset_id: i64,
    ) -> rusqlite::Result<Option<SourceTranscript>> {
        self.0
            .query_row(
                "SELECT provider, language_code, text, words_json FROM asset_transcripts
                 WHERE workspace_id = ?1 AND asset_id = ?2",
                params![workspace_id, asset_id],
                |row| {
                    let words: String = row.get(3)?;
                    Ok(SourceTranscript {
                        provider: row.get(0)?,
                        language_code: row.get(1)?,
                        text: row.get(2)?,
                        words: serde_json::from_str(&words).unwrap_or_default(),
                    })
                },
            )
            .optional()
    }

    /// Returns the cached transcript of a video or audio asset, transcribing it first if needed.
    pub fn transcribe_source_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        providers: &[String],
    ) -> Result<SourceTranscript, Box<dyn Error>> {
        if let Some(cached) = self.source_transcript(workspace_id, asset_id)? {
            return Ok(cached);
        }
        let kind: String = self.0.query_row(
            "SELECT kind FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, asset_id],
            |row| row.get(0),
        )?;
        if kind == "image" {
            return Err(io::Error::other("Images have no audio to transcribe").into());
        }
        let source = asset_path(&asset_root(database), workspace_id, asset_id);
        let duration = media_duration(&source)?;
        if duration > MAX_SOURCE_SECONDS {
            return Err(io::Error::other("The clip is too long to transcribe").into());
        }
        let job = tempfile::tempdir()?;
        let mut words = Vec::new();
        let mut text = Vec::new();
        let mut provider = String::new();
        let mut language_code = String::new();
        let mut offset = 0.0;
        while offset < duration.max(0.01) {
            let wave = job
                .path()
                .join(format!("chunk-{}.wav", (offset / CHUNK_SECONDS) as u32));
            extract_chunk(&source, offset, &wave)?;
            let transcript = self.transcribe_ordered_wav(
                workspace_id,
                providers.to_vec(),
                fs::File::open(&wave)?,
                false,
            )?;
            provider = transcript.provider;
            language_code = transcript.transcript.language_code;
            text.push(transcript.transcript.text);
            words.extend(
                transcript
                    .transcript
                    .words
                    .into_iter()
                    .map(|word| TranscriptWord {
                        start: word.start + offset,
                        end: word.end + offset,
                        ..word
                    }),
            );
            fs::remove_file(&wave)?;
            offset += CHUNK_SECONDS;
        }
        let stored = SourceTranscript {
            provider,
            language_code,
            text: text.join(" "),
            words,
        };
        self.0.execute(
            "INSERT OR REPLACE INTO asset_transcripts (workspace_id, asset_id, provider, language_code, text, words_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                workspace_id,
                asset_id,
                stored.provider,
                stored.language_code,
                stored.text,
                serde_json::to_string(&stored.words)?
            ],
        )?;
        Ok(stored)
    }
}

pub(crate) fn media_duration(path: &Path) -> io::Result<f64> {
    let output = Command::new(crate::media_tools::ffprobe())
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    let duration: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| io::Error::other("Could not read the clip duration"))?;
    if !output.status.success() || !duration.is_finite() || duration < 0.0 {
        return Err(io::Error::other("Could not read the clip duration"));
    }
    Ok(duration)
}

fn extract_chunk(source: &Path, offset: f64, wave: &Path) -> io::Result<()> {
    let status = Command::new(crate::media_tools::ffmpeg())
        .args([
            "-nostdin",
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-ss",
        ])
        .arg(format!("{offset:.3}"))
        .args(["-t"])
        .arg(format!("{CHUNK_SECONDS:.3}"))
        .arg("-i")
        .arg(source)
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
        .arg(crate::transcription::MAX_AUDIO_BYTES.to_string())
        .arg(wave)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() || wave.metadata()?.len() >= crate::transcription::MAX_AUDIO_BYTES {
        return Err(io::Error::other("Could not extract the clip's audio"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start: f64) -> TranscriptWord {
        TranscriptWord {
            text: text.into(),
            start,
            end: start + 0.3,
            speaker_id: None,
        }
    }

    #[test]
    fn cuts_never_split_a_word_and_keep_a_little_air() {
        let timed = |text: &str, start: f64, end: f64| TranscriptWord {
            text: text.into(),
            start,
            end,
            speaker_id: None,
        };
        let words = [
            timed("Ninety", 2.52, 2.9),
            timed("percent", 2.95, 3.4),
            timed("wasted.", 3.45, 5.36),
            timed("We", 6.08, 6.2),
            timed("fixed", 6.25, 6.74),
        ];
        // Inside "Ninety" and inside "wasted.": both edges move out, padded.
        let (start, end) = snap_cut(&words, 2.7, 5.0);
        assert!((start - 2.47).abs() < 1e-9 && (end - 5.44).abs() < 1e-9);
        // In the long pause: left alone.
        assert_eq!(snap_cut(&words, 5.7, 6.9), (5.7, 6.9));
        // The tail padding never reaches the next word.
        let (_, end) = snap_cut(&words, 2.0, 2.92);
        assert!((end - 2.95).abs() < 1e-9);
        // Edges barely inside a long word drop it, padding from the kept neighbour rather than
        // sitting on the dropped word's boundary.
        let (start, end) = snap_cut(&words, 5.3, 6.5);
        assert!((start - 6.03).abs() < 1e-9 && (end - 6.82).abs() < 1e-9);
        let (_, end) = snap_cut(&words, 2.0, 3.51);
        assert!((end - 3.45).abs() < 1e-9);
    }

    #[test]
    fn lines_break_at_pauses_and_speaker_changes() {
        let spoken = |text: &str, start: f64, end: f64, speaker: &str| TranscriptWord {
            text: text.into(),
            start,
            end,
            speaker_id: Some(speaker.into()),
        };
        let transcript = SourceTranscript {
            provider: "fixture".into(),
            language_code: "en".into(),
            text: String::new(),
            words: vec![
                spoken("We", 2.52, 2.7, "a"),
                spoken("fixed", 2.75, 3.1, "a"),
                spoken("this", 3.7, 4.0, "a"),
                spoken("Nice", 4.1, 4.4, "b"),
            ],
        };
        assert_eq!(
            transcript.timed_lines(10_000),
            "[002.52-003.10] S0 We fixed\n[003.70-004.00] S0 this\n[004.10-004.40] S1 Nice\n"
        );
    }

    #[test]
    fn what_the_prompt_leaves_out_is_split_into_whole_line_parts() {
        let words: Vec<TranscriptWord> = (0..40)
            .map(|index| word(&format!("Sentence{index}."), index as f64 * 60.0))
            .collect();
        let transcript = SourceTranscript {
            provider: "fixture".into(),
            language_code: "en".into(),
            text: String::new(),
            words,
        };
        let full = transcript.timed_lines(usize::MAX);
        let inline = transcript.timed_lines(100);
        let parts = transcript.remaining_parts(100, 120);
        assert!(parts.iter().all(|part| part.text.chars().count() <= 120));
        assert!(parts.iter().all(|part| part.text.ends_with('\n')));
        // Inline lines plus the parts cover the whole transcript, the cut line included.
        let joined: String = parts.iter().map(|part| part.text.as_str()).collect();
        assert!(full.ends_with(&joined));
        assert!(inline.len() + joined.len() >= full.len());
        assert_eq!(
            parts[0].from,
            joined.lines().next().unwrap()[1..]
                .split(['-', ']'])
                .next()
                .unwrap()
        );
        assert_eq!(parts.last().unwrap().to, "2340.30");
        assert!(transcript.remaining_parts(usize::MAX, 120).is_empty());
    }

    #[test]
    fn timed_lines_break_on_sentences_and_respect_the_limit() {
        let transcript = SourceTranscript {
            provider: "fixture".into(),
            language_code: "en".into(),
            text: String::new(),
            words: vec![
                word("The", 0.0),
                word("build", 0.4),
                word("is", 0.8),
                word("red.", 1.0),
                word("Day", 62.0),
                word("three", 62.5),
                word("?", 63.0),
            ],
        };
        assert_eq!(
            transcript.timed_lines(10_000),
            "[000.00-001.30] The build is red.\n[062.00-063.30] Day three?\n"
        );
        assert!(transcript.timed_lines(12).chars().count() <= 12);
        let plain = SourceTranscript {
            words: vec![],
            text: "No timings".into(),
            ..transcript
        };
        assert_eq!(plain.timed_lines(100), "No timings");
    }

    #[test]
    fn cached_transcripts_are_scoped_and_removed_with_their_asset() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let asset: i64 = store
            .0
            .query_row(
                "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size, notes)
             VALUES (?1, 'take.mp4', 'video', 1, '') RETURNING id",
                [workspace],
                |row| row.get(0),
            )
            .unwrap();
        store.0.execute(
            "INSERT INTO asset_transcripts (workspace_id, asset_id, provider, language_code, text, words_json)
             VALUES (?1, ?2, 'fixture', 'en', 'Hello', '[]')", params![workspace, asset]).unwrap();
        assert_eq!(
            store
                .source_transcript(workspace, asset)
                .unwrap()
                .unwrap()
                .text,
            "Hello"
        );
        assert!(store.source_transcript(other, asset).unwrap().is_none());
        store
            .0
            .execute("DELETE FROM workspace_assets WHERE id = ?1", [asset])
            .unwrap();
        assert!(store.source_transcript(workspace, asset).unwrap().is_none());
    }
}
