use rusqlite::{Row, params};
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WorkspaceFeedback {
    pub id: i64,
    pub title: String,
    pub markdown: String,
}

impl WorkspaceStore {
    pub fn list_workspace_feedback(
        &self,
        workspace_id: i64,
    ) -> rusqlite::Result<Vec<WorkspaceFeedback>> {
        self.0.prepare(
            "SELECT id, title, markdown FROM workspace_feedback WHERE workspace_id = ?1 ORDER BY id DESC",
        )?.query_map([workspace_id], read_feedback)?.collect()
    }

    pub fn add_workspace_feedback(
        &self,
        workspace_id: i64,
        title: &str,
        markdown: &str,
    ) -> rusqlite::Result<WorkspaceFeedback> {
        if markdown.trim().is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "INSERT INTO workspace_feedback (workspace_id, title, markdown) VALUES (?1, ?2, ?3)
             RETURNING id, title, markdown",
            params![workspace_id, title.trim(), markdown],
            read_feedback,
        )
    }

    pub fn update_workspace_feedback(
        &self,
        workspace_id: i64,
        feedback_id: i64,
        title: &str,
        markdown: &str,
    ) -> rusqlite::Result<WorkspaceFeedback> {
        if markdown.trim().is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "UPDATE workspace_feedback SET title = ?3, markdown = ?4 WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, title, markdown",
            params![workspace_id, feedback_id, title.trim(), markdown],
            read_feedback,
        )
    }
}

fn read_feedback(row: &Row<'_>) -> rusqlite::Result<WorkspaceFeedback> {
    Ok(WorkspaceFeedback {
        id: row.get(0)?,
        title: row.get(1)?,
        markdown: row.get(2)?,
    })
}
