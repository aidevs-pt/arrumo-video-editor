use rusqlite::params;
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, Serialize)]
pub struct GenerationNotification {
    pub id: i64,
    pub run_id: i64,
    pub kind: String,
    pub created_at: i64,
}

impl WorkspaceStore {
    pub fn generation_notifications(
        &self,
        workspace_id: i64,
        after_id: i64,
    ) -> rusqlite::Result<Vec<GenerationNotification>> {
        self.0
            .prepare(
                "SELECT notice.id, notice.run_id, notice.kind, notice.created_at
                 FROM generation_notifications AS notice
                 JOIN generation_runs AS run ON run.id = notice.run_id
                 WHERE run.workspace_id = ?1 AND notice.id > ?2
                 ORDER BY notice.id LIMIT 100",
            )?
            .query_map(params![workspace_id, after_id], |row| {
                Ok(GenerationNotification {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect()
    }
}
