//! Clips uploaded for a video take most of a workspace's space, so once the video is finished
//! they are deleted unless something else still needs them.

use std::{error::Error, path::Path};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{AssetRemoval, WorkspaceStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageSettings {
    /// Delete the clips uploaded for a video once it is finished.
    pub delete_used_footage: bool,
}

impl WorkspaceStore {
    pub fn storage_settings(&self) -> rusqlite::Result<StorageSettings> {
        let delete: Option<bool> = self
            .0
            .query_row(
                "SELECT delete_used_footage FROM storage_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(StorageSettings {
            delete_used_footage: delete.unwrap_or(true),
        })
    }

    pub fn set_storage_settings(
        &self,
        settings: &StorageSettings,
    ) -> rusqlite::Result<StorageSettings> {
        self.0.execute(
            "INSERT INTO storage_settings (id, delete_used_footage) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET delete_used_footage = excluded.delete_used_footage",
            [settings.delete_used_footage],
        )?;
        self.storage_settings()
    }

    /// The uploaded clips of a finished run that are gone, so it can no longer be made again from them.
    pub fn run_footage_removed(&self, workspace_id: i64, run_id: i64) -> rusqlite::Result<bool> {
        self.0.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM generation_runs AS run, json_each(run.snapshot_json, '$.draft.source_ids') AS source
                 WHERE run.workspace_id = ?1 AND run.id = ?2 AND NOT EXISTS (
                     SELECT 1 FROM workspace_assets AS asset
                     WHERE asset.workspace_id = run.workspace_id AND asset.id = source.value
                 )
             )",
            params![workspace_id, run_id],
            |row| row.get(0),
        )
    }

    /// Deletes the clips uploaded for a finished run, when the creator keeps that setting on.
    /// A clip another draft, an open video, a finished video's media or an animation still uses
    /// stays. Answers the IDs it deleted.
    pub fn release_finished_footage(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
    ) -> Result<Vec<i64>, Box<dyn Error>> {
        if !self.storage_settings()?.delete_used_footage {
            return Ok(Vec::new());
        }
        let candidates: Vec<(i64, String)> = self
            .0
            .prepare(
                "SELECT asset.id, asset.name
                 FROM generation_runs AS run
                 JOIN json_each(run.snapshot_json, '$.draft.source_ids') AS source
                 JOIN workspace_assets AS asset
                   ON asset.workspace_id = run.workspace_id AND asset.id = source.value
                 WHERE run.workspace_id = ?1 AND run.id = ?2 AND run.state = 'completed'
                   AND asset.footage = 1
                   AND NOT EXISTS (
                       SELECT 1 FROM video_draft_sources AS other
                       WHERE other.workspace_id = run.workspace_id AND other.asset_id = asset.id
                         AND other.draft_id IS NOT run.draft_id
                   )",
            )?
            .query_map(params![workspace_id, run_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut removed = Vec::new();
        for (asset_id, name) in candidates {
            if self.remove_asset_explained(database, workspace_id, asset_id, &name)?
                == AssetRemoval::Removed
            {
                removed.push(asset_id);
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::{DraftSettings, ReplayMode, RunOptions, StorageSettings, WorkspaceStore};

    fn clip(store: &WorkspaceStore, workspace: i64, name: &str) -> i64 {
        store
            .0
            .query_row(
                "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size)
                 VALUES (?1, ?2, 'video', 1) RETURNING id",
                params![workspace, name],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn finish(store: &WorkspaceStore, run: i64) {
        store
            .0
            .execute(
                "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                [run],
            )
            .unwrap();
    }

    #[test]
    fn a_finished_video_deletes_the_clips_only_it_needed() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let own = clip(&store, workspace, "own.mp4");
        let shared = clip(&store, workspace, "shared.mp4");
        let draft = |store: &mut WorkspaceStore, title: &str, sources: Vec<i64>| {
            store
                .create_video_draft(
                    workspace,
                    DraftSettings {
                        title: title.into(),
                        profile_id: None,
                        intent: String::new(),
                        source_ids: sources,
                    },
                )
                .unwrap()
                .id
        };
        let first = draft(&mut store, "First", vec![own, shared]);
        draft(&mut store, "Second", vec![shared]);
        let run = store
            .enqueue_video_draft(workspace, first, "first", &RunOptions::default())
            .unwrap()
            .id;
        assert_eq!(
            store
                .release_finished_footage(&database, workspace, run)
                .unwrap(),
            Vec::<i64>::new(),
            "nothing goes while the video is still being made"
        );

        finish(&store, run);
        assert_eq!(
            store
                .release_finished_footage(&database, workspace, run)
                .unwrap(),
            [own],
            "a clip another draft still uses stays"
        );
        assert!(store.run_footage_removed(workspace, run).unwrap());
        assert!(
            store
                .replay_generation(workspace, run, "again", ReplayMode::Captured)
                .is_err(),
            "a video whose clips are gone can't be made again from them"
        );
    }

    #[test]
    fn the_creator_can_keep_used_clips() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        assert!(store.storage_settings().unwrap().delete_used_footage);
        store
            .set_storage_settings(&StorageSettings {
                delete_used_footage: false,
            })
            .unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let own = clip(&store, workspace, "own.mp4");
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Kept".into(),
                    profile_id: None,
                    intent: String::new(),
                    source_ids: vec![own],
                },
            )
            .unwrap()
            .id;
        let run = store
            .enqueue_video_draft(workspace, draft, "kept", &RunOptions::default())
            .unwrap()
            .id;
        finish(&store, run);
        assert!(
            store
                .release_finished_footage(&database, workspace, run)
                .unwrap()
                .is_empty()
        );
        assert!(!store.run_footage_removed(workspace, run).unwrap());
    }
}
