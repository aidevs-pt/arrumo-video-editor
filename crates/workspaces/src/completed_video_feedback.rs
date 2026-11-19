use rusqlite::{Row, params};
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, Serialize)]
pub struct CompletedVideoFeedback {
    pub id: i64,
    pub run_id: i64,
    pub result_id: Option<i64>,
    pub markdown: String,
    pub created_at: i64,
}

impl WorkspaceStore {
    pub fn add_completed_video_feedback(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: Option<i64>,
        markdown: &str,
    ) -> rusqlite::Result<CompletedVideoFeedback> {
        if markdown.trim().is_empty() || markdown.contains('\0') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "INSERT INTO completed_video_feedback (run_id, result_id, markdown)
             SELECT id, ?3, ?4 FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'
             RETURNING id, run_id, result_id, markdown, created_at",
            params![workspace_id, run_id, result_id, markdown],
            read_feedback,
        )
    }

    pub(crate) fn completed_video_feedback(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<CompletedVideoFeedback>> {
        self.0.prepare(
            "SELECT feedback.id, feedback.run_id, feedback.result_id, feedback.markdown, feedback.created_at
             FROM completed_video_feedback AS feedback
             JOIN generation_runs AS run ON run.id = feedback.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 ORDER BY feedback.id",
        )?.query_map(params![workspace_id, run_id], read_feedback)?.collect()
    }

    pub(crate) fn draft_delivery_feedback(
        &self,
        workspace_id: i64,
        draft_id: i64,
    ) -> rusqlite::Result<Vec<CompletedVideoFeedback>> {
        self.0.prepare(
            "SELECT feedback.id, feedback.run_id, feedback.result_id, feedback.markdown, feedback.created_at
             FROM completed_video_feedback AS feedback
             JOIN generation_runs AS run ON run.id = feedback.run_id
             WHERE run.workspace_id = ?1 AND run.draft_id = ?2 ORDER BY feedback.id",
        )?.query_map(params![workspace_id, draft_id], read_feedback)?.collect()
    }
}

fn read_feedback(row: &Row<'_>) -> rusqlite::Result<CompletedVideoFeedback> {
    Ok(CompletedVideoFeedback {
        id: row.get(0)?,
        run_id: row.get(1)?,
        result_id: row.get(2)?,
        markdown: row.get(3)?,
        created_at: row.get(4)?,
    })
}
