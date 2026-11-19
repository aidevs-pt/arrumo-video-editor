use std::{error::Error, io, path::Path};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

use crate::WorkspaceStore;
use crate::generation_worker::LEASE_SECONDS;

#[derive(Debug, Serialize)]
pub struct MediaWork {
    pub stage: String,
    pub attempt: i64,
}

impl WorkspaceStore {
    pub fn process_media_stage(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        preview_edit: Option<&Value>,
    ) -> Result<Option<MediaWork>, Box<dyn Error>> {
        if preview_edit.is_some_and(|edit| !edit.is_object()) {
            return Err(io::Error::other("Preview edit must be an object").into());
        }
        let claim: Option<MediaWork> = self
            .0
            .query_row(
                "UPDATE generation_runs
                 SET state = 'running', attempt = attempt + 1, lease_until = unixepoch() + ?3
                 WHERE workspace_id = ?1 AND id = ?2
                   AND (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
                   AND CASE json_extract(plan_json, '$.steps[' || stage_index || '].stage')
                       WHEN 'preview' THEN ?4 AND json_array_length(snapshot_json, '$.draft.source_ids') > 0
                       WHEN 'final_render' THEN NOT ?4 AND json_array_length(snapshot_json, '$.draft.source_ids') > 0
                       WHEN 'verify_delivery' THEN NOT ?4
                       ELSE 0
                   END
                 RETURNING json_extract(plan_json, '$.steps[' || stage_index || '].stage'), attempt",
                params![workspace_id, run_id, LEASE_SECONDS, preview_edit.is_some()],
                |row| Ok(MediaWork { stage: row.get(0)?, attempt: row.get(1)? }),
            )
            .optional()?;
        let Some(work) = claim else {
            return Ok(None);
        };

        let attempt = work.attempt;
        let result = match preview_edit {
            Some(edit) => self
                .render_preview_videos(database, workspace_id, run_id, attempt, edit)
                .map(|_| ()),
            None if work.stage == "final_render" => self
                .render_primary_videos(database, workspace_id, run_id, attempt)
                .map(|_| ()),
            None => self
                .verify_generation_delivery(database, workspace_id, run_id, attempt)
                .map(|_| ()),
        };
        if let Err(error) = result {
            // Keep native paths and external command details out of persisted activity.
            let detail = match work.stage.as_str() {
                "preview" => {
                    "Preview rendering failed. Check the edit, managed sources, and FFmpeg installation."
                }
                "final_render" => {
                    "Final rendering failed. Check the accepted edit, managed sources, and FFmpeg installation."
                }
                _ => "Delivery verification failed. Check the rendered videos and subtitle files.",
            };
            match self.fail_generation(workspace_id, run_id, attempt, detail) {
                Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(record_error) => {
                    return Err(io::Error::other(format!(
                        "{error}. Could not record the worker failure: {record_error}"
                    ))
                    .into());
                }
            }
            return Err(error);
        }
        Ok(Some(work))
    }
}
