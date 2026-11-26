use std::{error::Error, io, path::Path};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

use crate::generation_worker::LEASE_SECONDS;
use crate::{ProductionStage, WorkspaceStore};

#[derive(Debug, Serialize)]
pub struct MediaWork {
    pub stage: String,
    pub attempt: i64,
}

impl WorkspaceStore {
    /// Renders the next queued Final render from recorded footage, using the approved preview.
    pub fn process_next_final_render(&self, database: &Path) -> Result<bool, Box<dyn Error>> {
        let candidates: Vec<(i64, i64)> = self
            .0
            .prepare(
                "SELECT workspace_id, id FROM generation_runs
                 WHERE (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
                   AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'final_render'
                 ORDER BY id",
            )?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (workspace_id, run_id) in candidates {
            if self
                .process_media_stage_at(
                    database,
                    workspace_id,
                    run_id,
                    ProductionStage::FinalRender,
                )?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn process_media_stage(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        preview_edit: Option<&Value>,
    ) -> Result<Option<MediaWork>, Box<dyn Error>> {
        self.execute_media_stage(database, workspace_id, run_id, preview_edit, None)
    }

    pub fn process_media_stage_at(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        stage: ProductionStage,
    ) -> Result<Option<MediaWork>, Box<dyn Error>> {
        let (stage_name, edit) = match stage {
            ProductionStage::Preview => ("preview", Some(serde_json::json!({}))),
            ProductionStage::FinalRender => ("final_render", None),
            ProductionStage::VerifyDelivery => ("verify_delivery", None),
            _ => return Err(io::Error::other("This stage has no media worker").into()),
        };
        self.execute_media_stage(
            database,
            workspace_id,
            run_id,
            edit.as_ref(),
            Some(stage_name),
        )
    }

    fn execute_media_stage(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        preview_edit: Option<&Value>,
        expected_stage: Option<&str>,
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
                   AND (?5 IS NULL OR json_extract(plan_json, '$.steps[' || stage_index || '].stage') = ?5)
                   AND CASE json_extract(plan_json, '$.steps[' || stage_index || '].stage')
                       WHEN 'preview' THEN ?4 AND json_array_length(snapshot_json, '$.draft.source_ids') > 0
                       WHEN 'final_render' THEN NOT ?4
                       WHEN 'verify_delivery' THEN NOT ?4
                       ELSE 0
                   END
                 RETURNING json_extract(plan_json, '$.steps[' || stage_index || '].stage'), attempt",
                params![workspace_id, run_id, LEASE_SECONDS, preview_edit.is_some(), expected_stage],
                |row| Ok(MediaWork { stage: row.get(0)?, attempt: row.get(1)? }),
            )
            .optional()?;
        let Some(work) = claim else {
            return Ok(None);
        };

        let attempt = work.attempt;
        let timing = crate::telemetry::time_step(&work.stage, &[]);
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
        timing.finish(result.is_ok());
        if let Err(error) = result {
            crate::telemetry::warn(
                "A generation step failed",
                &[("stage", work.stage.clone().into())],
            );
            let summary = match work.stage.as_str() {
                "preview" => {
                    "Preview rendering failed. Check the edit, managed sources, and FFmpeg installation."
                }
                "final_render" => {
                    "Final rendering failed. Check the accepted edit, managed sources, and FFmpeg installation."
                }
                _ => "Delivery verification failed. Check the rendered videos and subtitle files.",
            };
            return match self.fail_generation_with_error(
                workspace_id,
                run_id,
                attempt,
                summary,
                &*error,
            ) {
                Ok(_) => Err(crate::Reported::new(&*error).into()),
                Err(rusqlite::Error::QueryReturnedNoRows) => Err(error),
                Err(record_error) => Err(io::Error::other(format!(
                    "{error}. Could not record the worker failure: {record_error}"
                ))
                .into()),
            };
        }
        Ok(Some(work))
    }
}
