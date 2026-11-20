use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionKind {
    Animation,
    Transition,
}

impl MotionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Animation => "animation",
            Self::Transition => "transition",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotionLibrarySettings {
    pub profile_id: Option<i64>,
    pub kind: MotionKind,
    pub title: String,
    pub usage_markdown: String,
    pub preview_asset_id: i64,
    pub thumbnail_asset_id: Option<i64>,
    pub sound_asset_id: Option<i64>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MotionLibraryItem {
    pub id: i64,
    pub workspace_id: i64,
    pub settings: MotionLibrarySettings,
}

const VALID_MEDIA: &str = "EXISTS (SELECT 1 FROM workspace_assets
                           WHERE workspace_id = ?1 AND id = ?6 AND kind = 'video' AND superseded = 0)
               AND (?7 IS NULL OR EXISTS (SELECT 1 FROM workspace_assets
                           WHERE workspace_id = ?1 AND id = ?7 AND kind = 'image' AND superseded = 0))
               AND (?8 IS NULL OR EXISTS (SELECT 1 FROM workspace_assets
                           WHERE workspace_id = ?1 AND id = ?8 AND kind = 'audio' AND superseded = 0))";

impl WorkspaceStore {
    pub fn create_motion_library_item(
        &self,
        workspace_id: i64,
        settings: &MotionLibrarySettings,
    ) -> rusqlite::Result<MotionLibraryItem> {
        self.0.query_row(
            &format!(
                "INSERT INTO motion_library
                (workspace_id, profile_id, kind, title, usage_markdown,
                 preview_asset_id, thumbnail_asset_id, sound_asset_id)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
             WHERE {VALID_MEDIA}
             RETURNING id, workspace_id, profile_id, kind, title, usage_markdown,
                       preview_asset_id, thumbnail_asset_id, sound_asset_id"
            ),
            params![
                workspace_id,
                settings.profile_id,
                settings.kind.as_str(),
                settings.title.trim(),
                settings.usage_markdown,
                settings.preview_asset_id,
                settings.thumbnail_asset_id,
                settings.sound_asset_id,
            ],
            read_item,
        )
    }

    pub fn update_motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
        settings: &MotionLibrarySettings,
    ) -> rusqlite::Result<MotionLibraryItem> {
        self.0.query_row(
            &format!(
                "UPDATE motion_library
                 SET profile_id = ?2, kind = ?3, title = ?4, usage_markdown = ?5,
                     preview_asset_id = ?6, thumbnail_asset_id = ?7, sound_asset_id = ?8
                 WHERE workspace_id = ?1 AND id = ?9 AND {VALID_MEDIA}
                 RETURNING id, workspace_id, profile_id, kind, title, usage_markdown,
                           preview_asset_id, thumbnail_asset_id, sound_asset_id"
            ),
            params![
                workspace_id,
                settings.profile_id,
                settings.kind.as_str(),
                settings.title.trim(),
                settings.usage_markdown,
                settings.preview_asset_id,
                settings.thumbnail_asset_id,
                settings.sound_asset_id,
                item_id,
            ],
            read_item,
        )
    }

    pub fn duplicate_motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
        title: &str,
    ) -> rusqlite::Result<MotionLibraryItem> {
        self.0.query_row(
            "INSERT INTO motion_library
                (workspace_id, profile_id, kind, title, usage_markdown,
                 preview_asset_id, thumbnail_asset_id, sound_asset_id)
             SELECT workspace_id, profile_id, kind, ?3, usage_markdown,
                    preview_asset_id, thumbnail_asset_id, sound_asset_id
             FROM motion_library WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, profile_id, kind, title, usage_markdown,
                       preview_asset_id, thumbnail_asset_id, sound_asset_id",
            params![workspace_id, item_id, title.trim()],
            read_item,
        )
    }

    pub fn delete_motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
        confirmed_title: &str,
    ) -> rusqlite::Result<bool> {
        self.0
            .execute(
                "DELETE FROM motion_library WHERE workspace_id = ?1 AND id = ?2 AND title = ?3",
                params![workspace_id, item_id, confirmed_title],
            )
            .map(|count| count != 0)
    }

    pub fn list_motion_library(
        &self,
        workspace_id: i64,
        profile_id: Option<i64>,
    ) -> rusqlite::Result<Vec<MotionLibraryItem>> {
        if let Some(profile_id) = profile_id {
            self.0.query_row(
                "SELECT id FROM profiles WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id, profile_id],
                |_| Ok(()),
            )?;
        }
        self.0
            .prepare(
                "SELECT id, workspace_id, profile_id, kind, title, usage_markdown,
                    preview_asset_id, thumbnail_asset_id, sound_asset_id
             FROM motion_library
             WHERE workspace_id = ?1 AND (profile_id IS NULL OR profile_id = ?2)
             ORDER BY id",
            )?
            .query_map(params![workspace_id, profile_id], read_item)?
            .collect()
    }
}

fn read_item(row: &Row<'_>) -> rusqlite::Result<MotionLibraryItem> {
    Ok(MotionLibraryItem {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        settings: MotionLibrarySettings {
            profile_id: row.get(2)?,
            kind: match row.get::<_, String>(3)?.as_str() {
                "animation" => MotionKind::Animation,
                "transition" => MotionKind::Transition,
                _ => return Err(rusqlite::Error::InvalidQuery),
            },
            title: row.get(4)?,
            usage_markdown: row.get(5)?,
            preview_asset_id: row.get(6)?,
            thumbnail_asset_id: row.get(7)?,
            sound_asset_id: row.get(8)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_root;
    use std::fs;

    #[test]
    fn library_validates_media_and_follows_workspace_asset_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let foreign = store.create("Other").unwrap().id;
        let profile = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let other_profile = store
            .create_profile(workspace, "Shorts", "", None)
            .unwrap()
            .id;
        let path = temp.path().join("overlay.mp4");
        let root = asset_root(&database);
        fs::write(&path, b"preview fixture").unwrap();
        let preview = store.import_asset(&root, workspace, &path).unwrap();
        let foreign_preview = store.import_asset(&root, foreign, &path).unwrap();
        let mut settings = MotionLibrarySettings {
            profile_id: Some(profile),
            kind: MotionKind::Animation,
            title: "  Overlay  ".into(),
            usage_markdown: "Use over the product shot.".into(),
            preview_asset_id: foreign_preview.id,
            thumbnail_asset_id: None,
            sound_asset_id: None,
        };
        assert!(
            store
                .create_motion_library_item(workspace, &settings)
                .is_err()
        );
        settings.preview_asset_id = preview.id;
        settings.sound_asset_id = Some(preview.id);
        assert!(
            store
                .create_motion_library_item(workspace, &settings)
                .is_err()
        );
        settings.sound_asset_id = None;
        let item = store
            .create_motion_library_item(workspace, &settings)
            .unwrap();
        assert_eq!(item.settings.title, "Overlay");
        settings.profile_id = None;
        settings.kind = MotionKind::Transition;
        let shared = store
            .create_motion_library_item(workspace, &settings)
            .unwrap();
        assert_eq!(
            store.list_motion_library(workspace, None).unwrap(),
            vec![shared]
        );
        assert_eq!(
            store
                .list_motion_library(workspace, Some(profile))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            store
                .list_motion_library(workspace, Some(other_profile))
                .unwrap()
                .len(),
            1
        );
        assert!(store.list_motion_library(foreign, Some(profile)).is_err());
        assert!(
            !store
                .remove_workspace_asset(&database, workspace, preview.id, &preview.name)
                .unwrap()
        );
        let replacement = store
            .replace_asset(&root, workspace, preview.id, &path)
            .unwrap();
        drop(store);
        let store = WorkspaceStore::open(&database).unwrap();
        let items = store.list_motion_library(workspace, Some(profile)).unwrap();
        assert!(
            items
                .iter()
                .all(|item| item.settings.preview_asset_id == replacement.id)
        );
    }
}
