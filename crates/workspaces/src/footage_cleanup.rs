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

    /// Saves the settings and, when deleting used clips is on, deletes those already waiting.
    /// The setting is saved even when a clip can't be deleted yet.
    pub fn apply_storage_settings(
        &self,
        database: &Path,
        settings: &StorageSettings,
    ) -> rusqlite::Result<StorageSettings> {
        let saved = self.set_storage_settings(settings)?;
        if saved.delete_used_footage {
            let _ = self.sweep_finished_footage(database);
        }
        Ok(saved)
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
        self.release_footage(database, Some((workspace_id, run_id)))
    }

    /// The same for every finished video, so clips of videos finished before this setting, or
    /// still in use when theirs finished, go once nothing needs them.
    pub fn sweep_finished_footage(&self, database: &Path) -> Result<Vec<i64>, Box<dyn Error>> {
        self.release_footage(database, None)
    }

    fn release_footage(
        &self,
        database: &Path,
        run: Option<(i64, i64)>,
    ) -> Result<Vec<i64>, Box<dyn Error>> {
        if !self.storage_settings()?.delete_used_footage {
            return Ok(Vec::new());
        }
        let (workspace_id, run_id) = run.unzip();
        let candidates: Vec<(i64, i64, String)> = self
            .0
            .prepare(
                "SELECT DISTINCT asset.workspace_id, asset.id, asset.name
                 FROM generation_runs AS run
                 JOIN json_each(run.snapshot_json, '$.draft.source_ids') AS source
                 JOIN workspace_assets AS asset
                   ON asset.workspace_id = run.workspace_id AND asset.id = source.value
                 WHERE (?1 IS NULL OR (run.workspace_id = ?1 AND run.id = ?2))
                   AND run.state = 'completed' AND asset.footage = 1
                   AND NOT EXISTS (
                       SELECT 1 FROM video_draft_sources AS other
                       WHERE other.workspace_id = run.workspace_id AND other.asset_id = asset.id
                         AND NOT EXISTS (
                             SELECT 1 FROM generation_runs AS made
                             WHERE made.workspace_id = other.workspace_id
                               AND made.draft_id = other.draft_id AND made.state = 'completed'
                         )
                   )",
            )?
            .query_map(params![workspace_id, run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut removed = Vec::new();
        for (workspace_id, asset_id, name) in candidates {
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

    use crate::{
        DraftSettings, ReplayMode, RunOptions, StorageSettings, WorkspaceStore, asset_path,
        asset_root, frame_cache,
    };

    struct Studio {
        _directory: tempfile::TempDir,
        database: std::path::PathBuf,
        store: WorkspaceStore,
        workspace: i64,
    }

    fn studio() -> Studio {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        Studio {
            _directory: directory,
            database,
            store,
            workspace,
        }
    }

    impl Studio {
        /// An uploaded clip with a real file and a cached still, as the app leaves them.
        fn upload(&mut self, name: &str) -> i64 {
            let source = self.database.with_file_name(name);
            std::fs::write(&source, b"video bytes").unwrap();
            let root = asset_root(&self.database);
            let id = self
                .store
                .import_footage(&root, self.workspace, &source)
                .unwrap()
                .id;
            let frames = frame_cache(&self.database).join(self.workspace.to_string());
            std::fs::create_dir_all(&frames).unwrap();
            std::fs::write(frames.join(format!("{id}-11-1.jpg")), b"still").unwrap();
            id
        }

        fn library_clip(&mut self, name: &str) -> i64 {
            let source = self.database.with_file_name(name);
            std::fs::write(&source, b"video bytes").unwrap();
            let root = asset_root(&self.database);
            self.store
                .import_asset(&root, self.workspace, &source)
                .unwrap()
                .id
        }

        fn made(&mut self, title: &str, sources: Vec<i64>) -> (i64, i64) {
            let draft = self
                .store
                .create_video_draft(
                    self.workspace,
                    DraftSettings {
                        title: title.into(),
                        profile_id: None,
                        intent: String::new(),
                        source_ids: sources,
                    },
                )
                .unwrap()
                .id;
            let run = self
                .store
                .enqueue_video_draft(self.workspace, draft, title, &RunOptions::default())
                .unwrap()
                .id;
            (draft, run)
        }

        fn finish(&self, run: i64) {
            self.store
                .0
                .execute(
                    "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                    [run],
                )
                .unwrap();
        }

        fn release(&self, run: i64) -> Vec<i64> {
            self.store
                .release_finished_footage(&self.database, self.workspace, run)
                .unwrap()
        }

        fn on_disk(&self, asset: i64) -> bool {
            asset_path(&asset_root(&self.database), self.workspace, asset).exists()
        }

        fn still_cached(&self, asset: i64) -> bool {
            frame_cache(&self.database)
                .join(self.workspace.to_string())
                .join(format!("{asset}-11-1.jpg"))
                .exists()
        }
    }

    #[test]
    fn finished_clips_leave_the_disk_and_the_video_says_so() {
        let mut studio = studio();
        let own = studio.upload("own.mp4");
        let shared = studio.upload("shared.mp4");
        let kept = studio.upload("kept.mp4");
        let (_, first) = studio.made("First", vec![own, shared]);
        let (_, second) = studio.made("Second", vec![shared]);
        let draft_only = studio.store.create_video_draft(
            studio.workspace,
            DraftSettings {
                title: "Later".into(),
                profile_id: None,
                intent: String::new(),
                source_ids: vec![kept],
            },
        );
        assert!(draft_only.is_ok());

        studio.finish(first);
        assert_eq!(
            studio.release(first),
            [own],
            "the open second video still needs the shared clip"
        );
        assert!(!studio.on_disk(own) && !studio.still_cached(own));
        assert!(studio.on_disk(shared) && studio.still_cached(shared));
        assert!(
            studio
                .store
                .run_footage_removed(studio.workspace, first)
                .unwrap()
        );
        assert!(
            studio
                .store
                .replay_generation(studio.workspace, first, "again", ReplayMode::Captured)
                .is_err(),
            "a video whose clips are gone can't be made again from them"
        );

        studio.finish(second);
        assert_eq!(
            studio.release(second),
            [shared],
            "once both are finished it goes"
        );
        assert!(!studio.on_disk(shared));
        assert!(studio.on_disk(kept), "a draft not yet made keeps its clip");
    }

    #[test]
    fn library_clips_and_animation_clips_stay() {
        let mut studio = studio();
        let library = studio.library_clip("logo-sting.mp4");
        let in_animation = studio.upload("demo.mp4");
        let (_, run) = studio.made("Uses both", vec![library, in_animation]);
        let preview = studio.library_clip("preview.mp4");
        studio
            .store
            .0
            .execute(
                "INSERT INTO motion_library (workspace_id, kind, title, usage_markdown, preview_asset_id, spec_json)
                 VALUES (?1, 'animation', 'Demo card', '', ?2, ?3)",
                params![
                    studio.workspace,
                    preview,
                    serde_json::json!({"layers": [{"type": "video", "assetId": in_animation}]}).to_string()
                ],
            )
            .unwrap();
        studio.finish(run);
        assert!(studio.release(run).is_empty());
        assert!(studio.on_disk(library) && studio.on_disk(in_animation));
    }

    #[test]
    fn the_sweep_catches_videos_finished_earlier_and_respects_the_setting() {
        let mut studio = studio();
        let old = studio.upload("old.mp4");
        let (_, run) = studio.made("Finished before", vec![old]);
        studio.finish(run);
        let off = StorageSettings {
            delete_used_footage: false,
        };
        studio
            .store
            .apply_storage_settings(&studio.database, &off)
            .unwrap();
        assert!(
            studio
                .store
                .sweep_finished_footage(&studio.database)
                .unwrap()
                .is_empty()
        );
        assert!(studio.on_disk(old));
        assert!(
            !studio
                .store
                .run_footage_removed(studio.workspace, run)
                .unwrap()
        );

        let on = StorageSettings {
            delete_used_footage: true,
        };
        studio
            .store
            .apply_storage_settings(&studio.database, &on)
            .unwrap();
        assert!(
            !studio.on_disk(old),
            "turning it on deletes clips already waiting"
        );
        assert!(
            studio
                .store
                .sweep_finished_footage(&studio.database)
                .unwrap()
                .is_empty()
        );
    }
}
