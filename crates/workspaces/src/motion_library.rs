use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{GenerationPlan, ProductionStage, WorkspaceStore, format_deliveries::read_settings};

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
                           WHERE workspace_id = ?1 AND id = ?6 AND kind = 'video'
                             AND (superseded = 0 OR ?9 OR id = (
                                 SELECT preview_asset_id FROM motion_library WHERE workspace_id = ?1 AND id = ?10
                             )))
               AND (?7 IS NULL OR EXISTS (SELECT 1 FROM workspace_assets
                           WHERE workspace_id = ?1 AND id = ?7 AND kind = 'image'
                             AND (superseded = 0 OR ?9 OR id = (
                                 SELECT thumbnail_asset_id FROM motion_library WHERE workspace_id = ?1 AND id = ?10
                             ))))
               AND (?8 IS NULL OR EXISTS (SELECT 1 FROM workspace_assets
                           WHERE workspace_id = ?1 AND id = ?8 AND kind = 'audio'
                             AND (superseded = 0 OR ?9 OR id = (
                                 SELECT sound_asset_id FROM motion_library WHERE workspace_id = ?1 AND id = ?10
                             ))))";

pub(crate) fn validate_motion_results(
    tx: &Transaction<'_>,
    workspace_id: i64,
    value: &serde_json::Value,
) -> rusqlite::Result<()> {
    let entries: Vec<MotionLibrarySettings> = serde_json::from_value(value.clone())
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let mut statement = tx.prepare(&format!(
        "SELECT trim(?4) <> '' AND {VALID_MEDIA}
         AND (?2 IS NULL OR EXISTS (SELECT 1 FROM profiles WHERE workspace_id = ?1 AND id = ?2))
         AND ?3 IN ('animation', 'transition') AND ?5 IS NOT NULL"
    ))?;
    for settings in entries {
        let valid: bool = statement.query_row(
            params![
                workspace_id,
                settings.profile_id,
                settings.kind.as_str(),
                settings.title.trim(),
                settings.usage_markdown,
                settings.preview_asset_id,
                settings.thumbnail_asset_id,
                settings.sound_asset_id,
                false,
                None::<i64>,
            ],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

impl WorkspaceStore {
    pub fn create_motion_library_item(
        &self,
        workspace_id: i64,
        settings: &MotionLibrarySettings,
    ) -> rusqlite::Result<MotionLibraryItem> {
        self.insert_motion_library_item(workspace_id, settings, false)
    }

    fn insert_motion_library_item(
        &self,
        workspace_id: i64,
        settings: &MotionLibrarySettings,
        captured: bool,
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
                captured,
                None::<i64>,
            ],
            read_item,
        )
    }

    pub fn save_motion_result(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        index: i64,
    ) -> rusqlite::Result<MotionLibraryItem> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let (stage_index, decision, plan, result): (
            i64,
            Option<String>,
            GenerationPlan,
            serde_json::Value,
        ) = tx.query_row(
            "SELECT result.stage_index, result.decision, run.plan_json, result.result_json
                 FROM production_results AS result
                 JOIN generation_runs AS run ON run.id = result.run_id
                 WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3
                   AND result.stage_index < run.stage_index",
            params![workspace_id, run_id, result_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    read_settings(row.get(2)?, 2)?,
                    read_settings(row.get(3)?, 3)?,
                ))
            },
        )?;
        if !plan.steps.get(stage_index as usize).is_some_and(|step| {
            step.stage == ProductionStage::Assets
                && (decision.as_deref() == Some("approved")
                    || (!step.review_required && decision.is_none()))
        }) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let position = usize::try_from(index).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let settings = result
            .get("motion_items")
            .and_then(|items| items.get(position))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let settings: MotionLibrarySettings = serde_json::from_value(settings.clone())
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let item =
            self.save_captured_motion_item(workspace_id, run_id, result_id, index, &settings)?;
        tx.commit()?;
        Ok(item)
    }

    /// Motion results of a run already in the library: (result ID, item index, profile ID).
    pub fn saved_run_motion(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<(i64, i64, Option<i64>)>> {
        self.0
            .prepare(
                "SELECT source_result_id, source_motion_index, profile_id FROM motion_library
                 WHERE workspace_id = ?1 AND source_run_id = ?2
                   AND source_result_id IS NOT NULL AND source_motion_index IS NOT NULL",
            )?
            .query_map(params![workspace_id, run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect()
    }

    /// A motion item an Assets-stage result proposes, read for previewing during review.
    pub fn run_motion_item(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        index: usize,
    ) -> rusqlite::Result<MotionLibrarySettings> {
        let item: String = self.0.query_row(
            "SELECT json_extract(result.result_json, '$.motion_items[' || ?4 || ']')
             FROM production_results AS result JOIN generation_runs AS run ON run.id = result.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3",
            params![workspace_id, run_id, result_id, index as i64],
            |row| row.get(0),
        )?;
        serde_json::from_str(&item).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }

    pub(crate) fn save_captured_motion_item(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        index: i64,
        settings: &MotionLibrarySettings,
    ) -> rusqlite::Result<MotionLibraryItem> {
        let existing = self.0.query_row(
            "SELECT id, workspace_id, profile_id, kind, title, usage_markdown,
                    preview_asset_id, thumbnail_asset_id, sound_asset_id
             FROM motion_library WHERE workspace_id = ?1 AND source_result_id = ?2 AND source_motion_index = ?3",
            params![workspace_id, result_id, index],
            read_item,
        ).optional()?;
        if let Some(item) = existing {
            return Ok(item);
        }
        let item = self.insert_motion_library_item(workspace_id, settings, true)?;
        self.0.execute(
            "UPDATE motion_library SET source_run_id = ?2, source_result_id = ?3, source_motion_index = ?4,
                 spec_json = (SELECT json_extract(result_json, '$.motion_specs[' || ?4 || ']')
                              FROM production_results WHERE id = ?3)
             WHERE id = ?1",
            params![item.id, run_id, result_id, index],
        )?;
        Ok(item)
    }

    pub fn update_motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
        settings: &MotionLibrarySettings,
    ) -> rusqlite::Result<MotionLibraryItem> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        // A different preview, still or sound is a new version; a new name or note is not.
        let media_changed: bool = tx.query_row(
            "SELECT preview_asset_id IS NOT ?3 OR thumbnail_asset_id IS NOT ?4 OR sound_asset_id IS NOT ?5
             FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
            params![
                workspace_id,
                item_id,
                settings.preview_asset_id,
                settings.thumbnail_asset_id,
                settings.sound_asset_id
            ],
            |row| row.get(0),
        )?;
        if media_changed {
            crate::motion_versions::keep_current_version(&tx, workspace_id, item_id, "", None)?;
        }
        let item = tx.query_row(
            &format!(
                "UPDATE motion_library
                 SET profile_id = ?2, kind = ?3, title = ?4, usage_markdown = ?5,
                     preview_asset_id = ?6, thumbnail_asset_id = ?7, sound_asset_id = ?8
                 WHERE workspace_id = ?1 AND id = ?10 AND {VALID_MEDIA}
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
                false,
                item_id,
            ],
            read_item,
        )?;
        tx.execute(
            "DELETE FROM motion_profile_shares
             WHERE workspace_id = ?1 AND item_id = ?2 AND (?3 IS NULL OR profile_id = ?3)",
            params![workspace_id, item_id, settings.profile_id],
        )?;
        tx.commit()?;
        Ok(item)
    }

    pub fn duplicate_motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
        title: &str,
    ) -> rusqlite::Result<MotionLibraryItem> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let item = tx.query_row(
            "INSERT INTO motion_library
                (workspace_id, profile_id, kind, title, usage_markdown,
                 preview_asset_id, thumbnail_asset_id, sound_asset_id, spec_json)
             SELECT workspace_id, profile_id, kind, ?3, usage_markdown,
                    preview_asset_id, thumbnail_asset_id, sound_asset_id, spec_json
             FROM motion_library WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, profile_id, kind, title, usage_markdown,
                       preview_asset_id, thumbnail_asset_id, sound_asset_id",
            params![workspace_id, item_id, title.trim()],
            read_item,
        )?;
        tx.execute(
            "INSERT INTO motion_profile_shares (workspace_id, item_id, profile_id)
             SELECT workspace_id, ?3, profile_id FROM motion_profile_shares
             WHERE workspace_id = ?1 AND item_id = ?2",
            params![workspace_id, item_id, item.id],
        )?;
        tx.commit()?;
        Ok(item)
    }

    /// Deletes the item and its versions, then the media Arrumo made for them that nothing
    /// else uses.
    pub fn delete_motion_library_item(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
        item_id: i64,
        confirmed_title: &str,
    ) -> rusqlite::Result<bool> {
        let media: Vec<i64> = self
            .0
            .prepare(
                "SELECT value FROM (
                     SELECT preview_asset_id AS value FROM motion_library WHERE workspace_id = ?1 AND id = ?2 AND title = ?3
                     UNION SELECT thumbnail_asset_id FROM motion_library WHERE workspace_id = ?1 AND id = ?2 AND title = ?3
                     UNION SELECT version.preview_asset_id FROM motion_library_versions AS version
                         JOIN motion_library AS item ON item.workspace_id = version.workspace_id AND item.id = version.item_id
                         WHERE item.workspace_id = ?1 AND item.id = ?2 AND item.title = ?3
                     UNION SELECT version.thumbnail_asset_id FROM motion_library_versions AS version
                         JOIN motion_library AS item ON item.workspace_id = version.workspace_id AND item.id = version.item_id
                         WHERE item.workspace_id = ?1 AND item.id = ?2 AND item.title = ?3
                 ) WHERE value IS NOT NULL",
            )?
            .query_map(params![workspace_id, item_id, confirmed_title], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let deleted = self.0.execute(
            "DELETE FROM motion_library WHERE workspace_id = ?1 AND id = ?2 AND title = ?3",
            params![workspace_id, item_id, confirmed_title],
        )? != 0;
        if deleted {
            self.release_motion_media(database, workspace_id, &media);
        }
        Ok(deleted)
    }

    pub fn motion_library_item(
        &self,
        workspace_id: i64,
        item_id: i64,
    ) -> rusqlite::Result<MotionLibraryItem> {
        self.0.query_row(
            "SELECT id, workspace_id, profile_id, kind, title, usage_markdown,
                    preview_asset_id, thumbnail_asset_id, sound_asset_id
             FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, item_id],
            read_item,
        )
    }

    pub fn motion_library_shares(
        &self,
        workspace_id: i64,
        item_id: i64,
    ) -> rusqlite::Result<Vec<i64>> {
        self.0.query_row(
            "SELECT id FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, item_id],
            |_| Ok(()),
        )?;
        self.0
            .prepare(
                "SELECT profile_id FROM motion_profile_shares
             WHERE workspace_id = ?1 AND item_id = ?2 ORDER BY profile_id",
            )?
            .query_map(params![workspace_id, item_id], |row| row.get(0))?
            .collect()
    }

    pub fn set_motion_library_shares(
        &self,
        workspace_id: i64,
        item_id: i64,
        profile_ids: &[i64],
    ) -> rusqlite::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let owner: Option<i64> = tx.query_row(
            "SELECT profile_id FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, item_id],
            |row| row.get(0),
        )?;
        if owner.is_none() && !profile_ids.is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        tx.execute(
            "DELETE FROM motion_profile_shares WHERE workspace_id = ?1 AND item_id = ?2",
            params![workspace_id, item_id],
        )?;
        for &profile_id in profile_ids {
            if Some(profile_id) == owner {
                return Err(rusqlite::Error::InvalidQuery);
            }
            tx.execute(
                "INSERT INTO motion_profile_shares (workspace_id, item_id, profile_id)
                 VALUES (?1, ?2, ?3)",
                params![workspace_id, item_id, profile_id],
            )?;
        }
        tx.commit()
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
             FROM motion_library AS item
             WHERE workspace_id = ?1 AND (profile_id IS NULL OR profile_id = ?2
                 OR EXISTS (SELECT 1 FROM motion_profile_shares AS share
                            WHERE share.workspace_id = ?1 AND share.item_id = item.id AND share.profile_id = ?2))
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
