//! Earlier states of library animations. A change made with the agent, or a new preview chosen
//! by hand, replaces the item and keeps what it replaced, so the creator can compare and go back.

use std::{error::Error, path::Path};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::{AssetRemoval, MotionKind, MotionLibrarySettings, WorkspaceStore};

/// How many earlier states an item keeps; the oldest go, with media nothing else uses.
pub(crate) const MAX_VERSIONS: i64 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MotionVersion {
    pub version: i64,
    pub current: bool,
    pub kind: MotionKind,
    pub title: String,
    pub usage_markdown: String,
    pub preview_asset_id: i64,
    pub thumbnail_asset_id: Option<i64>,
    pub sound_asset_id: Option<i64>,
    /// What the creator asked the agent to change, when an agent made this version.
    pub change_note: String,
    /// The earlier version this one brought back.
    pub restored_from: Option<i64>,
    /// When a later version replaced it; empty for the current one.
    pub replaced_at: Option<String>,
}

impl MotionVersion {
    pub fn settings(&self, profile_id: Option<i64>) -> MotionLibrarySettings {
        MotionLibrarySettings {
            profile_id,
            kind: self.kind,
            title: self.title.clone(),
            usage_markdown: self.usage_markdown.clone(),
            preview_asset_id: self.preview_asset_id,
            thumbnail_asset_id: self.thumbnail_asset_id,
            sound_asset_id: self.sound_asset_id,
        }
    }
}

/// Keeps the item's current state as a version before it changes, and drops versions past
/// [`MAX_VERSIONS`]. Answers the media of the dropped versions, to release after the commit.
pub(crate) fn keep_current_version(
    connection: &Connection,
    workspace_id: i64,
    item_id: i64,
    change_note: &str,
    restored_from: Option<i64>,
) -> rusqlite::Result<Vec<i64>> {
    let kept = connection.execute(
        "INSERT INTO motion_library_versions
            (workspace_id, item_id, version, kind, title, usage_markdown, preview_asset_id,
             thumbnail_asset_id, sound_asset_id, spec_json, component_json, change_note, restored_from)
         SELECT workspace_id, id, version, kind, title, usage_markdown, preview_asset_id,
                thumbnail_asset_id, sound_asset_id, spec_json, component_json, change_note, restored_from
         FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id, item_id],
    )?;
    if kept == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    connection.execute(
        "UPDATE motion_library SET version = version + 1, change_note = ?3, restored_from = ?4
         WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id, item_id, change_note.trim(), restored_from],
    )?;
    let dropped: Vec<(i64, Option<i64>, Option<i64>)> = connection
        .prepare(
            "DELETE FROM motion_library_versions
             WHERE workspace_id = ?1 AND item_id = ?2 AND version NOT IN (
                 SELECT version FROM motion_library_versions
                 WHERE workspace_id = ?1 AND item_id = ?2 ORDER BY version DESC LIMIT ?3
             )
             RETURNING preview_asset_id, thumbnail_asset_id, sound_asset_id",
        )?
        .query_map(params![workspace_id, item_id, MAX_VERSIONS], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(dropped
        .into_iter()
        .flat_map(|(preview, thumbnail, sound)| [Some(preview), thumbnail, sound])
        .flatten()
        .collect())
}

/// An asset plan as agents read it: the pinned designs are for rendering, not for the prompt.
pub(crate) fn plan_for_agent(plan: &serde_json::Value) -> serde_json::Value {
    let mut plan = plan.clone();
    if let Some(entries) = plan.as_array_mut() {
        for entry in entries
            .iter_mut()
            .filter_map(serde_json::Value::as_object_mut)
        {
            entry.remove("spec");
        }
    }
    plan
}

impl WorkspaceStore {
    /// The item's current state first, then the versions it replaced, newest first.
    pub fn motion_library_versions(
        &self,
        workspace_id: i64,
        item_id: i64,
    ) -> rusqlite::Result<Vec<MotionVersion>> {
        self.0
            .prepare(
                "SELECT version, 1, kind, title, usage_markdown, preview_asset_id, thumbnail_asset_id,
                        sound_asset_id, change_note, restored_from, NULL
                 FROM motion_library WHERE workspace_id = ?1 AND id = ?2
                 UNION ALL
                 SELECT * FROM (
                     SELECT version, 0, kind, title, usage_markdown, preview_asset_id, thumbnail_asset_id,
                            sound_asset_id, change_note, restored_from, replaced_at
                     FROM motion_library_versions WHERE workspace_id = ?1 AND item_id = ?2
                     AND EXISTS (SELECT 1 FROM motion_library WHERE workspace_id = ?1 AND id = ?2)
                     ORDER BY version DESC
                 )",
            )?
            .query_map(params![workspace_id, item_id], |row| {
                Ok(MotionVersion {
                    version: row.get(0)?,
                    current: row.get(1)?,
                    kind: match row.get::<_, String>(2)?.as_str() {
                        "transition" => MotionKind::Transition,
                        _ => MotionKind::Animation,
                    },
                    title: row.get(3)?,
                    usage_markdown: row.get(4)?,
                    preview_asset_id: row.get(5)?,
                    thumbnail_asset_id: row.get(6)?,
                    sound_asset_id: row.get(7)?,
                    change_note: row.get(8)?,
                    restored_from: row.get(9)?,
                    replaced_at: row.get(10)?,
                })
            })?
            .collect()
    }

    pub fn motion_library_version(
        &self,
        workspace_id: i64,
        item_id: i64,
        version: i64,
    ) -> rusqlite::Result<MotionVersion> {
        self.motion_library_versions(workspace_id, item_id)?
            .into_iter()
            .find(|entry| entry.version == version)
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    /// Brings an earlier version back as the item's current state. The state it replaces is
    /// kept as a version too, so restoring can itself be undone.
    pub fn restore_motion_library_version(
        &self,
        database: &Path,
        workspace_id: i64,
        item_id: i64,
        version: i64,
    ) -> Result<crate::MotionLibraryItem, Box<dyn Error>> {
        let tx = self.0.unchecked_transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT version FROM motion_library_versions
                 WHERE workspace_id = ?1 AND item_id = ?2 AND version = ?3",
                params![workspace_id, item_id, version],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(rusqlite::Error::QueryReturnedNoRows.into());
        }
        let dropped = keep_current_version(&tx, workspace_id, item_id, "", Some(version))?;
        tx.execute(
            "UPDATE motion_library AS item SET
                 kind = old.kind, title = old.title, usage_markdown = old.usage_markdown,
                 preview_asset_id = old.preview_asset_id, thumbnail_asset_id = old.thumbnail_asset_id,
                 sound_asset_id = old.sound_asset_id, spec_json = old.spec_json,
                 component_json = old.component_json
             FROM motion_library_versions AS old
             WHERE item.workspace_id = ?1 AND item.id = ?2
               AND old.workspace_id = ?1 AND old.item_id = ?2 AND old.version = ?3",
            params![workspace_id, item_id, version],
        )?;
        tx.commit()?;
        self.release_motion_media(database, workspace_id, &dropped);
        self.motion_library_item(workspace_id, item_id)
            .map_err(Into::into)
    }

    /// Copies the design of each library animation an asset plan places into the plan, so the
    /// video keeps it however the item changes later.
    pub(crate) fn pin_library_designs(
        &self,
        workspace_id: i64,
        result: &mut serde_json::Value,
    ) -> rusqlite::Result<()> {
        let Some(plan) = result["asset_plan"].as_array_mut() else {
            return Ok(());
        };
        // Templates were filled per use when the plan was made.
        for entry in plan.iter_mut().filter(|entry| entry.get("spec").is_none()) {
            let Some(id) = entry["reference"]
                .as_str()
                .and_then(|reference| reference.strip_prefix("motion-"))
                .and_then(|id| id.parse::<i64>().ok())
            else {
                continue;
            };
            let pinned: Option<(i64, Option<String>)> = self
                .0
                .query_row(
                    "SELECT version, spec_json FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
                    params![workspace_id, id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((version, spec)) = pinned {
                entry["version"] = serde_json::json!(version);
                if let Some(spec) =
                    spec.and_then(|spec| serde_json::from_str::<serde_json::Value>(&spec).ok())
                {
                    entry["spec"] = spec;
                }
            }
        }
        Ok(())
    }

    /// Deletes media Arrumo made for animations once no item, version or video uses it.
    pub(crate) fn release_motion_media(&self, database: &Path, workspace_id: i64, ids: &[i64]) {
        for &asset_id in ids {
            let name: Option<String> = self
                .0
                .query_row(
                    "SELECT name FROM workspace_assets
                     WHERE workspace_id = ?1 AND id = ?2 AND generated = 1",
                    params![workspace_id, asset_id],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten();
            if let Some(name) = name {
                // Media still in use stays; it is released with its last user.
                let _: Option<AssetRemoval> = self
                    .remove_asset_explained(database, workspace_id, asset_id, &name)
                    .ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use serde_json::json;

    use super::{MAX_VERSIONS, plan_for_agent};
    use crate::{
        AssetRemoval, MotionKind, MotionLibrarySettings, WorkspaceStore, asset_path, asset_root,
    };

    struct Library {
        _directory: tempfile::TempDir,
        database: std::path::PathBuf,
        store: WorkspaceStore,
        workspace: i64,
    }

    fn library() -> Library {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        Library {
            _directory: directory,
            database,
            store,
            workspace,
        }
    }

    impl Library {
        /// A video with a real file; `generated` media is what Arrumo rendered.
        fn video(&self, name: &str, generated: bool) -> i64 {
            let id: i64 = self
                .store
                .0
                .query_row(
                    "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size, generated)
                     VALUES (?1, ?2, 'video', 1, ?3) RETURNING id",
                    params![self.workspace, name, generated],
                    |row| row.get(0),
                )
                .unwrap();
            let path = asset_path(&asset_root(&self.database), self.workspace, id);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"video").unwrap();
            id
        }

        fn on_disk(&self, asset: i64) -> bool {
            asset_path(&asset_root(&self.database), self.workspace, asset).exists()
        }

        fn settings(&self, title: &str, preview: i64) -> MotionLibrarySettings {
            MotionLibrarySettings {
                profile_id: None,
                kind: MotionKind::Animation,
                title: title.into(),
                usage_markdown: String::new(),
                preview_asset_id: preview,
                thumbnail_asset_id: None,
                sound_asset_id: None,
            }
        }

        fn versions(&self, item: i64) -> Vec<(i64, bool, i64)> {
            self.store
                .motion_library_versions(self.workspace, item)
                .unwrap()
                .into_iter()
                .map(|entry| (entry.version, entry.current, entry.preview_asset_id))
                .collect()
        }
    }

    #[test]
    fn new_media_keeps_the_previous_state_and_restoring_is_undoable() {
        let lib = library();
        let first = lib.video("first.mp4", true);
        let second = lib.video("second.mp4", true);
        let item = lib
            .store
            .create_motion_library_item(lib.workspace, &lib.settings("Card", first))
            .unwrap()
            .id;

        lib.store
            .update_motion_library_item(lib.workspace, item, &lib.settings("Card renamed", first))
            .unwrap();
        assert_eq!(
            lib.versions(item),
            [(1, true, first)],
            "a new name is not a new version"
        );

        lib.store
            .update_motion_library_item(lib.workspace, item, &lib.settings("Card", second))
            .unwrap();
        assert_eq!(lib.versions(item), [(2, true, second), (1, false, first)]);

        let restored = lib
            .store
            .restore_motion_library_version(&lib.database, lib.workspace, item, 1)
            .unwrap();
        assert_eq!(restored.settings.preview_asset_id, first);
        assert_eq!(
            lib.versions(item),
            [(3, true, first), (2, false, second), (1, false, first)]
        );
        let current = &lib
            .store
            .motion_library_versions(lib.workspace, item)
            .unwrap()[0];
        assert_eq!(current.restored_from, Some(1));
        assert!(
            lib.store
                .restore_motion_library_version(&lib.database, lib.workspace, item, 3)
                .is_err(),
            "the current one is not a version"
        );

        let removal = lib
            .store
            .remove_asset_explained(&lib.database, lib.workspace, second, "second.mp4")
            .unwrap();
        assert!(
            matches!(removal, AssetRemoval::InUse { .. }),
            "a version's media is still in use"
        );
        assert!(lib.on_disk(second));
    }

    #[test]
    fn only_the_latest_versions_stay_and_their_rendered_media_goes() {
        let lib = library();
        let chosen = lib.video("from-assets.mp4", false);
        let item = lib
            .store
            .create_motion_library_item(lib.workspace, &lib.settings("Card", chosen))
            .unwrap()
            .id;
        let mut rendered = Vec::new();
        for round in 0..=MAX_VERSIONS + 1 {
            let preview = lib.video(&format!("render-{round}.mp4"), true);
            rendered.push(preview);
            let tx = lib.store.0.unchecked_transaction().unwrap();
            let dropped =
                super::keep_current_version(&tx, lib.workspace, item, "brighter", None).unwrap();
            tx.execute(
                "UPDATE motion_library SET preview_asset_id = ?2 WHERE id = ?1",
                params![item, preview],
            )
            .unwrap();
            tx.commit().unwrap();
            lib.store
                .release_motion_media(&lib.database, lib.workspace, &dropped);
        }
        let versions = lib.versions(item);
        assert_eq!(
            versions.len() as i64,
            MAX_VERSIONS + 1,
            "current plus the cap"
        );
        assert!(
            lib.on_disk(chosen),
            "media chosen from Assets is never deleted"
        );
        assert!(
            !lib.on_disk(rendered[0]),
            "the oldest render dropped with its version"
        );
        assert!(rendered[2..].iter().all(|&id| lib.on_disk(id)));
        assert_eq!(
            lib.store
                .motion_library_versions(lib.workspace, item)
                .unwrap()[0]
                .change_note,
            "brighter"
        );

        let latest = *rendered.last().unwrap();
        assert!(
            lib.store
                .delete_motion_library_item(&lib.database, lib.workspace, item, "Card")
                .unwrap()
        );
        assert!(
            !lib.on_disk(latest) && !lib.on_disk(rendered[2]),
            "deleting the item releases its renders"
        );
        assert!(lib.on_disk(chosen));
    }

    #[test]
    fn a_planned_video_keeps_the_design_it_planned_with() {
        let lib = library();
        let preview = lib.video("card.mp4", true);
        let item = lib
            .store
            .create_motion_library_item(lib.workspace, &lib.settings("Card", preview))
            .unwrap()
            .id;
        lib.store
            .0
            .execute(
                "UPDATE motion_library SET spec_json = ?2 WHERE id = ?1",
                params![item, json!({"title": "Card v1"}).to_string()],
            )
            .unwrap();
        let mut result = json!({"asset_plan": [
            {"reference": format!("motion-{item}"), "name": "Card"},
            {"reference": "asset-4", "name": "Logo"}
        ]});
        lib.store
            .pin_library_designs(lib.workspace, &mut result)
            .unwrap();
        assert_eq!(result["asset_plan"][0]["spec"]["title"], "Card v1");
        assert_eq!(result["asset_plan"][0]["version"], 1);
        assert!(result["asset_plan"][1].get("spec").is_none());
        let shown = plan_for_agent(&result["asset_plan"]);
        assert!(shown[0].get("spec").is_none() && shown[0]["version"] == 1);
    }
}
