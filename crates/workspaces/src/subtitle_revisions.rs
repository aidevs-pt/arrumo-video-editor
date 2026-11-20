use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::{WorkspaceStore, delivery_verification::verify_subtitle_cues};

pub const MAX_SUBTITLE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
pub struct SubtitleRevision {
    pub revision: i64,
    pub subtitles: String,
}

impl WorkspaceStore {
    pub fn completed_subtitles(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        revision: Option<i64>,
    ) -> Result<SubtitleRevision, Box<dyn Error>> {
        let (_, original) =
            self.verified_delivery_files(database, workspace_id, run_id, format_id)?;
        if revision != Some(0) {
            let edited = self.read_edited_subtitles(run_id, format_id, revision)?;
            if let Some(edited) = edited {
                return Ok(edited);
            }
            if revision.is_some() {
                return Err(rusqlite::Error::QueryReturnedNoRows.into());
            }
        }
        Ok(SubtitleRevision {
            revision: 0,
            subtitles: read_subtitle_file(&original)?,
        })
    }

    pub(crate) fn read_edited_subtitles(
        &self,
        run_id: i64,
        format_id: i64,
        revision: Option<i64>,
    ) -> rusqlite::Result<Option<SubtitleRevision>> {
        self.0
            .query_row(
                "SELECT revision, subtitles FROM subtitle_revisions
                 WHERE run_id = ?1 AND format_id = ?2 AND (?3 IS NULL OR revision = ?3)
                 ORDER BY revision DESC LIMIT 1",
                params![run_id, format_id, revision],
                |row| {
                    Ok(SubtitleRevision {
                        revision: row.get(0)?,
                        subtitles: row.get(1)?,
                    })
                },
            )
            .optional()
    }

    pub fn revise_completed_subtitles(
        &self,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        expected_revision: i64,
        subtitles: &str,
    ) -> Result<i64, Box<dyn Error>> {
        if subtitles.is_empty()
            || subtitles.len() as u64 > MAX_SUBTITLE_BYTES
            || subtitles.contains('\0')
        {
            return Err(io::Error::other("Invalid subtitle text").into());
        }
        let duration = self
            .verified_delivery(workspace_id, run_id)?
            .verified_outputs
            .into_iter()
            .find(|output| output.format_id == format_id)
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?
            .duration_seconds;
        let temporary = tempfile::tempdir()?;
        File::create(temporary.path().join("revision.srt"))?.write_all(subtitles.as_bytes())?;
        let cue_count = i64::try_from(verify_subtitle_cues(
            temporary.path(),
            "revision.srt",
            duration,
        )?)?;

        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        self.verified_delivery(workspace_id, run_id)?;
        let current: i64 = tx.query_row(
            "SELECT coalesce(max(revision), 0) FROM subtitle_revisions WHERE run_id = ?1 AND format_id = ?2",
            params![run_id, format_id], |row| row.get(0),
        )?;
        if current != expected_revision {
            return Err(io::Error::other("Subtitle revision changed before save").into());
        }
        let revision = current
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Subtitle revision limit reached"))?;
        tx.execute(
            "INSERT INTO subtitle_revisions (run_id, format_id, revision, subtitles, cue_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, format_id, revision, subtitles, cue_count],
        )?;
        tx.commit()?;
        Ok(revision)
    }
}

pub fn read_subtitle_file(path: &Path) -> io::Result<String> {
    let mut subtitles = String::new();
    File::open(path)?
        .take(MAX_SUBTITLE_BYTES + 1)
        .read_to_string(&mut subtitles)?;
    if subtitles.is_empty() || subtitles.len() as u64 > MAX_SUBTITLE_BYTES {
        return Err(io::Error::other("Invalid subtitle file size"));
    }
    Ok(subtitles)
}
