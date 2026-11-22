use rusqlite::params;
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, Serialize)]
pub struct GenerationNotification {
    pub id: i64,
    pub run_id: i64,
    pub kind: String,
    pub created_at: i64,
    pub read_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GenerationInbox {
    pub reviews: Vec<ReviewNotice>,
    pub completions: Vec<CompletedNotice>,
}

#[derive(Debug, Serialize)]
pub struct ReviewNotice {
    pub run_id: i64,
    pub title: String,
    /// The draft's profile, when it still exists.
    pub profile: Option<String>,
    /// When the review was requested, in Unix seconds.
    pub at: i64,
}

#[derive(Debug, Serialize)]
pub struct CompletedNotice {
    pub run_id: i64,
    pub title: String,
    pub unread: bool,
    pub profile: Option<String>,
    /// When the run finished, in Unix seconds.
    pub at: i64,
}

const PROFILE_NAME: &str = "(SELECT name FROM profiles
      WHERE workspace_id = run.workspace_id
        AND id = json_extract(run.snapshot_json, '$.draft.profile_id'))";

impl WorkspaceStore {
    pub fn generation_inbox(&self, workspace_id: i64) -> rusqlite::Result<GenerationInbox> {
        let reviews = self
            .0
            .prepare(&format!(
                "SELECT run.id, COALESCE(run.display_title, json_extract(run.snapshot_json, '$.draft.title')),
                        {PROFILE_NAME},
                        COALESCE((SELECT MAX(created_at) FROM generation_notifications
                                  WHERE run_id = run.id AND kind = 'review_required'), unixepoch())
                 FROM generation_runs AS run
                 WHERE run.workspace_id = ?1 AND run.state = 'paused' AND run.pending_review_id IS NOT NULL
                 ORDER BY run.id DESC"
            ))?
            .query_map([workspace_id], |row| {
                Ok(ReviewNotice { run_id: row.get(0)?, title: row.get(1)?, profile: row.get(2)?, at: row.get(3)? })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let completions = self
            .0
            .prepare(&format!(
                "SELECT run.id, COALESCE(run.display_title, json_extract(run.snapshot_json, '$.draft.title')),
                        notice.read_at IS NULL, {PROFILE_NAME}, notice.created_at
                 FROM generation_notifications AS notice
                 JOIN generation_runs AS run ON run.id = notice.run_id
                 WHERE run.workspace_id = ?1 AND run.state = 'completed' AND notice.kind = 'completed'
                 ORDER BY notice.id DESC LIMIT 10"
            ))?
            .query_map([workspace_id], |row| {
                Ok(CompletedNotice {
                    run_id: row.get(0)?,
                    title: row.get(1)?,
                    unread: row.get(2)?,
                    profile: row.get(3)?,
                    at: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(GenerationInbox {
            reviews,
            completions,
        })
    }

    pub fn read_completion_notification(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<bool> {
        self.0
            .execute(
                "UPDATE generation_notifications SET read_at = unixepoch()
             WHERE run_id = ?2 AND kind = 'completed' AND read_at IS NULL
               AND EXISTS (SELECT 1 FROM generation_runs
                           WHERE id = ?2 AND workspace_id = ?1 AND state = 'completed')",
                params![workspace_id, run_id],
            )
            .map(|changed| changed > 0)
    }

    /// Marks every completion notice in the workspace as read. Returns how many changed.
    pub fn read_all_completion_notifications(&self, workspace_id: i64) -> rusqlite::Result<usize> {
        self.0.execute(
            "UPDATE generation_notifications SET read_at = unixepoch()
             WHERE kind = 'completed' AND read_at IS NULL
               AND run_id IN (SELECT id FROM generation_runs WHERE workspace_id = ?1)",
            [workspace_id],
        )
    }

    pub fn generation_notifications(
        &self,
        workspace_id: i64,
        after_id: i64,
    ) -> rusqlite::Result<Vec<GenerationNotification>> {
        self.0
            .prepare(
                "SELECT notice.id, notice.run_id, notice.kind, notice.created_at, notice.read_at
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
                    read_at: row.get(4)?,
                })
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunOptions};

    #[test]
    fn inbox_names_the_profile_and_marks_everything_read() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        store
            .0
            .execute(
                "INSERT INTO profiles (workspace_id, name) VALUES (?1, 'Shorts')",
                [workspace],
            )
            .unwrap();
        let profile = store.0.last_insert_rowid();
        for (ws, key, profile_id) in [
            (workspace, "a", Some(profile)),
            (workspace, "b", Some(profile)),
            (other, "c", None),
        ] {
            let draft = store
                .create_video_draft(
                    ws,
                    DraftSettings {
                        title: "Cube".into(),
                        intent: "Brief".into(),
                        source_ids: vec![],
                        profile_id,
                    },
                )
                .unwrap();
            let run = store
                .enqueue_video_draft(ws, draft.id, key, &RunOptions::default())
                .unwrap();
            store
                .0
                .execute(
                    "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                    [run.id],
                )
                .unwrap();
        }
        let inbox = store.generation_inbox(workspace).unwrap();
        assert_eq!(inbox.completions.len(), 2);
        assert!(inbox.completions.iter().all(|notice| notice.unread
            && notice.profile.as_deref() == Some("Shorts")
            && notice.at > 0));
        assert_eq!(
            store.read_all_completion_notifications(workspace).unwrap(),
            2
        );
        assert!(
            store
                .generation_inbox(workspace)
                .unwrap()
                .completions
                .iter()
                .all(|notice| !notice.unread)
        );
        assert!(store.generation_inbox(other).unwrap().completions[0].unread);
    }
}
