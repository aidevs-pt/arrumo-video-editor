use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{WorkspaceStore, format_deliveries::read_settings};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplementalTextKind {
    Title,
    Description,
    Tags,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupplementalText {
    pub kind: SupplementalTextKind,
    pub format_id: Option<i64>,
    pub text: String,
}

impl SupplementalText {
    pub(crate) fn filename(&self, index: usize) -> String {
        let kind = match self.kind {
            SupplementalTextKind::Title => "title",
            SupplementalTextKind::Description => "description",
            SupplementalTextKind::Tags => "tags",
        };
        format!("supplemental-{kind}-{}.txt", index + 1)
    }
}

pub(crate) fn validate_supplemental_text(
    connection: &rusqlite::Connection,
    run_id: i64,
    value: &Value,
) -> rusqlite::Result<()> {
    let mut formats = connection.prepare(
        "SELECT json_extract(format.value, '$.format.id')
         FROM generation_runs run, json_each(run.snapshot_json, '$.formats') format
         WHERE run.id = ?1",
    )?;
    let formats = formats
        .query_map([run_id], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
    let entries: Vec<SupplementalText> =
        serde_json::from_value(value.clone()).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if entries.len() > 100
        || entries.iter().map(|entry| entry.text.len()).sum::<usize>() > 64 * 1024
        || entries.iter().any(|entry| {
            entry.text.trim().is_empty()
                || entry.text.contains('\0')
                || entry.format_id.is_some_and(|id| !formats.contains(&id))
        })
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

impl WorkspaceStore {
    pub(crate) fn completed_supplemental_text(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<SupplementalText>> {
        self.0.query_row(
            "SELECT json_extract(result.result_json, '$.supplemental_text')
             FROM production_results result JOIN generation_runs run ON run.id = result.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND run.state = 'completed'
               AND json_extract(run.plan_json, '$.steps[' || result.stage_index || '].stage') = 'supplemental_outputs'
               AND (result.decision = 'approved' OR result.decision IS NULL)
             ORDER BY result.id DESC LIMIT 1",
            params![workspace_id, run_id],
            |row| read_settings(row.get(0)?, 0),
        ).optional().map(Option::unwrap_or_default)
    }
}
