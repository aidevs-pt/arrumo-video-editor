use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::format_deliveries::read_settings;
use crate::generation_queue::read_run;
use crate::{
    CompletedVideoFeedback, GenerationActivity, GenerationPlan, GenerationRun, ProductionProgress,
    VerifiedDelivery, WorkspaceStore,
};

#[derive(Debug, Serialize)]
pub struct CompletedVideo {
    pub run: GenerationRun,
    pub title: String,
    pub delivery: VerifiedDelivery,
    pub output_dates: BTreeMap<i64, String>,
    pub plan: GenerationPlan,
    pub production: ProductionProgress,
    pub activity: Vec<GenerationActivity>,
    pub feedback: Vec<CompletedVideoFeedback>,
}

impl WorkspaceStore {
    pub fn completed_video(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<CompletedVideo> {
        let (run, title, plan) = self.0.query_row(
            "SELECT id, draft_id, state, snapshot_json, pending_review_id,
                    COALESCE(display_title, json_extract(snapshot_json, '$.draft.title')), plan_json, display_title
             FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'",
            params![workspace_id, run_id],
            |row| Ok((read_run(row)?, row.get(5)?, read_settings(row.get(6)?, 6)?)),
        )?;
        Ok(CompletedVideo {
            run,
            title,
            delivery: self.verified_delivery(workspace_id, run_id)?,
            output_dates: self.run_output_dates(workspace_id, run_id)?,
            plan,
            production: self.production_progress(workspace_id, run_id)?,
            activity: self.generation_activity(workspace_id, run_id)?,
            feedback: self.completed_video_feedback(workspace_id, run_id)?,
        })
    }

    pub fn rename_completed_video(
        &self,
        workspace_id: i64,
        run_id: i64,
        title: &str,
    ) -> rusqlite::Result<String> {
        let title = title.trim();
        if title.is_empty() || title.contains('\0') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "UPDATE generation_runs SET display_title = ?3
             WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'
             RETURNING display_title",
            params![workspace_id, run_id, title],
            |row| row.get(0),
        )
    }
}
