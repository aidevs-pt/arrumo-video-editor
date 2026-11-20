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

#[derive(Debug, Serialize)]
pub struct SourceTranscript {
    pub provider: String,
    pub language_code: String,
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

impl SourceTranscript {
    /// Readable lines with the time each one starts, about one sentence each.
    pub fn timed_lines(&self, max_chars: usize) -> String {
        if self.words.is_empty() {
            return self.text.chars().take(max_chars).collect();
        }
        let mut lines = String::new();
        let mut current = String::new();
        let mut start = 0.0;
        for word in &self.words {
            if current.is_empty() {
                start = word.start;
            }
            if !current.is_empty() && !word.text.starts_with(|c: char| c.is_ascii_punctuation()) {
                current.push(' ');
            }
            current.push_str(word.text.trim());
            let ends = current.ends_with(['.', '?', '!']) || current.split(' ').count() >= 24;
            if ends {
                lines.push_str(&format!("[{}] {}\n", clock(start), current));
                current.clear();
            }
            if lines.len() > max_chars {
                break;
            }
        }
        if !current.is_empty() {
            lines.push_str(&format!("[{}] {}\n", clock(start), current));
        }
        lines.chars().take(max_chars).collect()
    }
}

fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
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

fn media_duration(path: &Path) -> io::Result<f64> {
    let output = Command::new("ffprobe")
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
    let status = Command::new("ffmpeg")
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
            "[0:00] The build is red.\n[1:02] Day three?\n"
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
