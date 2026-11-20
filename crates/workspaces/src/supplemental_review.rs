use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ReviewDecision, WorkspaceStore, format_deliveries::read_settings};

const SECTIONS: [&str; 2] = ["supplemental_text", "supplemental_media"];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupplementalReview {
    pub text: Vec<SupplementalDecision>,
    pub media: Vec<SupplementalDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_notes: Option<SupplementalItemNotes>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupplementalItemNotes {
    pub text: Vec<Option<String>>,
    pub media: Vec<Option<String>>,
}

impl SupplementalItemNotes {
    fn valid_for(&self, review: &SupplementalReview) -> bool {
        self.text.len() == review.text.len()
            && self.media.len() == review.media.len()
            && self
                .text
                .iter()
                .chain(&self.media)
                .flatten()
                .all(|note| note.len() <= 4096 && !note.contains('\0'))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupplementalDecision {
    Approved,
    Revise,
    Rejected,
}

impl SupplementalReview {
    fn sections(&self) -> impl Iterator<Item = (&'static str, &[SupplementalDecision])> {
        SECTIONS
            .into_iter()
            .zip([self.text.as_slice(), self.media.as_slice()])
    }
}

fn items<'a>(result: &'a Value, key: &str) -> rusqlite::Result<&'a [Value]> {
    match result.get(key) {
        None => Ok(&[]),
        Some(value) => value
            .as_array()
            .map(Vec::as_slice)
            .ok_or(rusqlite::Error::InvalidQuery),
    }
}

impl WorkspaceStore {
    pub fn review_supplemental_items(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        review: &SupplementalReview,
        notes: &str,
    ) -> rusqlite::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let result: Value = tx.query_row(
            "SELECT result.result_json FROM generation_runs run
             JOIN production_results result ON result.id = run.pending_review_id AND result.run_id = run.id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3 AND run.state = 'paused'
               AND result.stage_index = run.stage_index AND result.decision IS NULL
               AND json_extract(run.plan_json, '$.steps[' || run.stage_index || '].stage') = 'supplemental_outputs'",
            params![workspace_id, run_id, result_id],
            |row| read_settings(row.get(0)?, 0),
        )?;
        if (review.text.is_empty() && review.media.is_empty())
            || review
                .item_notes
                .as_ref()
                .is_some_and(|notes| !notes.valid_for(review))
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        for (key, decisions) in review.sections() {
            if items(&result, key)?.len() != decisions.len() {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        let approved = review.sections().all(|(_, decisions)| {
            decisions
                .iter()
                .all(|item| *item == SupplementalDecision::Approved)
        });
        let decision = if approved {
            ReviewDecision::Approved
        } else {
            ReviewDecision::Revise
        };
        self.review_production_result_in(
            &tx,
            workspace_id,
            run_id,
            result_id,
            decision,
            notes,
            None,
        )?;
        tx.execute(
            "UPDATE production_results SET supplemental_review_json = ?2 WHERE id = ?1",
            params![
                result_id,
                serde_json::to_string(review).map_err(|_| rusqlite::Error::InvalidQuery)?
            ],
        )?;
        tx.commit()
    }
}

struct PreviousReview {
    result_id: i64,
    result: Value,
    review: SupplementalReview,
    notes: Option<String>,
}

fn previous_review(
    connection: &rusqlite::Connection,
    run_id: i64,
    stage_index: i64,
) -> rusqlite::Result<Option<PreviousReview>> {
    connection
        .query_row(
            "SELECT id, result_json, supplemental_review_json, review_notes FROM (
             SELECT id, result_json, supplemental_review_json, review_notes, decision
             FROM production_results WHERE run_id = ?1 AND stage_index = ?2 ORDER BY id DESC LIMIT 1
         ) WHERE supplemental_review_json IS NOT NULL AND decision = 'revise'",
            params![run_id, stage_index],
            |row| {
                Ok(PreviousReview {
                    result_id: row.get(0)?,
                    result: read_settings(row.get(1)?, 1)?,
                    review: read_settings::<Option<SupplementalReview>>(row.get(2)?, 2)?
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                    notes: row.get(3)?,
                })
            },
        )
        .optional()
}

#[derive(Debug, Serialize)]
pub struct SupplementalRevisionRequest {
    pub result_id: i64,
    pub notes: Option<String>,
    pub text: Vec<RequestedReplacement>,
    pub media: Vec<RequestedReplacement>,
}

#[derive(Debug, Serialize)]
pub struct RequestedReplacement {
    pub index: usize,
    pub decision: SupplementalDecision,
    pub note: Option<String>,
    pub previous: Value,
}

pub(crate) fn supplemental_revision_request(
    connection: &rusqlite::Connection,
    run_id: i64,
    stage_index: i64,
) -> rusqlite::Result<Option<SupplementalRevisionRequest>> {
    let Some(previous) = previous_review(connection, run_id, stage_index)? else {
        return Ok(None);
    };
    let replacements = |key,
                        decisions: &[SupplementalDecision],
                        notes: Option<&[Option<String>]>|
     -> rusqlite::Result<Vec<RequestedReplacement>> {
        let originals = items(&previous.result, key)?;
        if decisions.len() != originals.len()
            || notes.is_some_and(|notes| notes.len() != decisions.len())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(originals
            .iter()
            .zip(decisions)
            .enumerate()
            .filter(|(_, (_, decision))| **decision != SupplementalDecision::Approved)
            .map(|(index, (entry, decision))| RequestedReplacement {
                index,
                decision: *decision,
                note: notes.and_then(|notes| notes[index].clone()),
                previous: entry.clone(),
            })
            .collect())
    };
    let notes = previous.review.item_notes.as_ref();
    Ok(Some(SupplementalRevisionRequest {
        result_id: previous.result_id,
        notes: previous.notes,
        text: replacements(
            SECTIONS[0],
            &previous.review.text,
            notes.map(|notes| notes.text.as_slice()),
        )?,
        media: replacements(
            SECTIONS[1],
            &previous.review.media,
            notes.map(|notes| notes.media.as_slice()),
        )?,
    }))
}

pub(crate) fn merge_supplemental_revisions(
    tx: &Transaction<'_>,
    run_id: i64,
    stage_index: i64,
    replacement: &Value,
) -> rusqlite::Result<Option<Value>> {
    let Some(PreviousReview {
        result: previous,
        review,
        ..
    }) = previous_review(tx, run_id, stage_index)?
    else {
        return Ok(None);
    };
    let mut merged = replacement.clone();
    for (key, decisions) in review.sections() {
        let originals = items(&previous, key)?;
        let mut changes = items(replacement, key)?.iter();
        let mut entries = Vec::with_capacity(originals.len());
        if decisions.len() != originals.len() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        for (original, decision) in originals.iter().zip(decisions) {
            if *decision == SupplementalDecision::Approved {
                entries.push(original.clone());
            } else {
                let changed = changes.next().ok_or(rusqlite::Error::InvalidQuery)?;
                if !changed.is_object()
                    || original["kind"] != changed["kind"]
                    || original["format_id"] != changed["format_id"]
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                if key == "supplemental_media" {
                    let same_kind: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM workspace_assets previous
                         JOIN workspace_assets next ON next.workspace_id = previous.workspace_id AND next.kind = previous.kind
                         WHERE previous.id = ?1 AND next.id = ?2)",
                        params![original["asset_id"].as_i64(), changed["asset_id"].as_i64()],
                        |row| row.get(0),
                    )?;
                    if !same_kind {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                }
                entries.push(changed.clone());
            }
        }
        if changes.next().is_some() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if !entries.is_empty() {
            merged[key] = Value::Array(entries);
        }
    }
    Ok(Some(merged))
}
