use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ReviewDecision, SupplementalDecision, WorkspaceStore, format_deliveries::read_settings,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewFormatDecision {
    pub format_id: i64,
    pub decision: SupplementalDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub(crate) fn preview_files(result: &Value) -> rusqlite::Result<BTreeMap<i64, String>> {
    let mut files = BTreeMap::new();
    for value in result
        .get("preview_videos")
        .and_then(Value::as_array)
        .ok_or(rusqlite::Error::InvalidQuery)?
    {
        let filename = value.as_str().ok_or(rusqlite::Error::InvalidQuery)?;
        let (attempt, format) = filename
            .strip_prefix("preview-")
            .and_then(|name| name.strip_suffix(".mp4"))
            .and_then(|name| name.split_once('-'))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let attempt: i64 = attempt.parse().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let format: i64 = format.parse().map_err(|_| rusqlite::Error::InvalidQuery)?;
        if attempt <= 0
            || format <= 0
            || filename != format!("preview-{attempt}-{format}.mp4")
            || files.insert(format, filename.to_owned()).is_some()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(files)
}

impl WorkspaceStore {
    pub fn review_preview_formats(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        decisions: &[PreviewFormatDecision],
        notes: &str,
    ) -> rusqlite::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let result: Value = tx.query_row(
            "SELECT result.result_json FROM generation_runs run
             JOIN production_results result ON result.run_id = run.id AND result.id = run.pending_review_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3 AND run.state = 'paused'
               AND result.stage_index = run.stage_index AND result.decision IS NULL
               AND json_extract(run.plan_json, '$.steps[' || run.stage_index || '].stage') = 'preview'",
            params![workspace_id, run_id, result_id],
            |row| read_settings(row.get(0)?, 0),
        )?;
        let files = preview_files(&result)?;
        let decisions_map = decisions
            .iter()
            .map(|entry| (entry.format_id, entry.decision))
            .collect::<BTreeMap<_, _>>();
        if decisions.iter().any(|entry| {
            entry
                .note
                .as_ref()
                .is_some_and(|note| note.len() > 4096 || note.contains('\0'))
        }) || files.is_empty()
            || decisions_map.len() != decisions.len()
            || !files.keys().eq(decisions_map.keys())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let formats = {
            let mut query = tx.prepare(
                "SELECT json_extract(value, '$.format.id') FROM generation_runs run,
                 json_each(run.snapshot_json, '$.formats') WHERE run.id = ?1 ORDER BY 1",
            )?;
            query
                .query_map([run_id], |row| row.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        if !files.keys().eq(formats.iter()) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let overall = if decisions_map
            .values()
            .all(|decision| *decision == SupplementalDecision::Approved)
        {
            ReviewDecision::Approved
        } else {
            ReviewDecision::Revise
        };
        self.review_production_result_in(
            &tx,
            workspace_id,
            run_id,
            result_id,
            overall,
            notes,
            None,
        )?;
        tx.execute(
            "UPDATE production_results SET preview_review_json = ?2 WHERE id = ?1",
            params![
                result_id,
                serde_json::to_string(decisions).map_err(|_| rusqlite::Error::InvalidQuery)?
            ],
        )?;
        tx.commit()
    }
}

pub(crate) struct PreviewRevision {
    result_id: i64,
    notes: Option<String>,
    previous: Value,
    pub approved: BTreeMap<i64, String>,
    pub unresolved: BTreeMap<i64, PreviewReplacement>,
}

pub(crate) fn preview_revision(
    connection: &rusqlite::Connection,
    run_id: i64,
    stage_index: i64,
) -> rusqlite::Result<Option<PreviewRevision>> {
    let prior: Option<(i64, Value, Vec<PreviewFormatDecision>, Option<String>)> = connection
        .query_row(
            "SELECT id, result_json, preview_review_json, review_notes FROM (
             SELECT id, result_json, preview_review_json, review_notes, decision FROM production_results
             WHERE run_id = ?1 AND stage_index = ?2 ORDER BY id DESC LIMIT 1
         ) WHERE decision = 'revise' AND preview_review_json IS NOT NULL",
            params![run_id, stage_index],
            |row| {
                Ok((
                    row.get(0)?,
                    read_settings(row.get(1)?, 1)?,
                    read_settings(row.get(2)?, 2)?,
                    row.get(3)?,
                ))
            },
        )
        .optional()?;
    let Some((result_id, previous, decisions, notes)) = prior else {
        return Ok(None);
    };
    let mut files = preview_files(&previous)?;
    let mut revision = PreviewRevision {
        result_id,
        notes,
        previous,
        approved: BTreeMap::new(),
        unresolved: BTreeMap::new(),
    };
    for entry in decisions {
        let filename = files
            .remove(&entry.format_id)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        if entry.decision == SupplementalDecision::Approved {
            revision.approved.insert(entry.format_id, filename);
        } else {
            let previous_cuts = effective_cuts(&revision.previous, entry.format_id);
            revision.unresolved.insert(
                entry.format_id,
                PreviewReplacement {
                    format_id: entry.format_id,
                    decision: entry.decision,
                    note: entry.note,
                    previous_file: filename,
                    previous_cuts,
                },
            );
        }
    }
    if !files.is_empty() || revision.unresolved.is_empty() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Some(revision))
}

#[derive(Debug, Serialize)]
pub struct PreviewReplacement {
    pub format_id: i64,
    pub decision: SupplementalDecision,
    pub note: Option<String>,
    pub previous_file: String,
    pub previous_cuts: Value,
}

#[derive(Debug, Serialize)]
pub struct PreviewRevisionRequest {
    pub result_id: i64,
    pub notes: Option<String>,
    pub formats: Vec<PreviewReplacement>,
}

pub(crate) fn preview_revision_request(
    connection: &rusqlite::Connection,
    run_id: i64,
    stage_index: i64,
) -> rusqlite::Result<Option<PreviewRevisionRequest>> {
    Ok(
        preview_revision(connection, run_id, stage_index)?.map(|revision| PreviewRevisionRequest {
            result_id: revision.result_id,
            notes: revision.notes,
            formats: revision.unresolved.into_values().collect(),
        }),
    )
}

pub(crate) fn merge_preview_revision(
    revision: PreviewRevision,
    attempt: i64,
    replacement: &Value,
) -> rusqlite::Result<Value> {
    let mut files = preview_files(replacement)?;
    if !files.keys().eq(revision.unresolved.keys())
        || files
            .iter()
            .any(|(format, filename)| *filename != format!("preview-{attempt}-{format}.mp4"))
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let mut merged = replacement.clone();
    let mut cuts = serde_json::Map::new();
    for &format in revision.approved.keys() {
        cuts.insert(
            format.to_string(),
            effective_cuts(&revision.previous, format),
        );
    }
    for &format in revision.unresolved.keys() {
        cuts.insert(format.to_string(), effective_cuts(replacement, format));
    }
    files.extend(revision.approved);
    merged["preview_videos"] = serde_json::json!(files.into_values().collect::<Vec<_>>());
    merged["format_cuts"] = Value::Object(cuts);
    Ok(merged)
}

fn effective_cuts(edit: &Value, format: i64) -> Value {
    edit.get("format_cuts")
        .and_then(|cuts| cuts.get(format.to_string()))
        .or_else(|| edit.get("cuts"))
        .cloned()
        .unwrap_or(Value::Null)
}
