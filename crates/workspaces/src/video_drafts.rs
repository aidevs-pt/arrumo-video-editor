use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DraftSettings {
    pub title: String,
    pub profile_id: Option<i64>,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub source_ids: Vec<i64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct VideoDraft {
    pub id: i64,
    #[serde(flatten)]
    pub settings: DraftSettings,
}

impl WorkspaceStore {
    pub fn create_video_draft(
        &mut self,
        workspace_id: i64,
        settings: DraftSettings,
    ) -> rusqlite::Result<VideoDraft> {
        self.save_video_draft(workspace_id, None, settings)
    }

    pub fn update_video_draft(
        &mut self,
        workspace_id: i64,
        draft_id: i64,
        settings: DraftSettings,
    ) -> rusqlite::Result<VideoDraft> {
        self.save_video_draft(workspace_id, Some(draft_id), settings)
    }

    fn save_video_draft(
        &mut self,
        workspace_id: i64,
        draft_id: Option<i64>,
        mut settings: DraftSettings,
    ) -> rusqlite::Result<VideoDraft> {
        settings.title = settings.title.trim().to_owned();
        if settings.title.is_empty()
            || settings.title.contains('\0')
            || (settings.source_ids.is_empty() && settings.intent.trim().is_empty())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = if let Some(id) = draft_id {
            tx.query_row(
                "UPDATE video_drafts SET profile_id = ?3, title = ?4, intent = ?5
                 WHERE workspace_id = ?1 AND id = ?2 RETURNING id",
                params![
                    workspace_id,
                    id,
                    settings.profile_id,
                    settings.title,
                    settings.intent
                ],
                |row| row.get::<_, i64>(0),
            )?;
            tx.execute(
                "DELETE FROM video_draft_sources WHERE workspace_id = ?1 AND draft_id = ?2",
                params![workspace_id, id],
            )?;
            id
        } else {
            tx.query_row(
                "INSERT INTO video_drafts (workspace_id, profile_id, title, intent)
                 VALUES (?1, ?2, ?3, ?4) RETURNING id",
                params![
                    workspace_id,
                    settings.profile_id,
                    settings.title,
                    settings.intent
                ],
                |row| row.get(0),
            )?
        };
        for (position, asset_id) in settings.source_ids.iter().enumerate() {
            tx.query_row(
                "INSERT INTO video_draft_sources (workspace_id, draft_id, asset_id, position)
                 SELECT workspace_id, ?2, id, ?4 FROM workspace_assets
                 WHERE workspace_id = ?1 AND id = ?3 AND kind = 'video' AND superseded = 0
                 RETURNING asset_id",
                params![workspace_id, id, asset_id, position as i64],
                |_| Ok(()),
            )?;
        }
        tx.commit()?;
        Ok(VideoDraft { id, settings })
    }

    pub fn delete_video_draft(
        &self,
        workspace_id: i64,
        draft_id: i64,
        confirmed_title: &str,
    ) -> rusqlite::Result<bool> {
        self.0
            .execute(
                "DELETE FROM video_drafts
             WHERE workspace_id = ?1 AND id = ?2 AND title = ?3
               AND NOT EXISTS (SELECT 1 FROM generation_runs
                               WHERE workspace_id = ?1 AND draft_id = ?2)",
                params![workspace_id, draft_id, confirmed_title],
            )
            .map(|deleted| deleted != 0)
    }

    pub fn list_video_drafts(&self, workspace_id: i64) -> rusqlite::Result<Vec<VideoDraft>> {
        self.0
            .prepare(
                "SELECT draft.id, draft.title, draft.profile_id, draft.intent,
                    (SELECT json_group_array(source.asset_id ORDER BY source.position)
                     FROM video_draft_sources AS source
                     WHERE source.workspace_id = draft.workspace_id AND source.draft_id = draft.id)
             FROM video_drafts AS draft WHERE draft.workspace_id = ?1 ORDER BY draft.id",
            )?
            .query_map([workspace_id], |row| {
                Ok(VideoDraft {
                    id: row.get(0)?,
                    settings: DraftSettings {
                        title: row.get(1)?,
                        profile_id: row.get(2)?,
                        intent: row.get(3)?,
                        source_ids: crate::format_deliveries::read_settings(row.get(4)?, 4)?,
                    },
                })
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_sources_and_rolls_back_invalid_groups() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let studio = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let profile = store
            .create_profile(studio, "Reviews", "", None)
            .unwrap()
            .id;
        let foreign_profile = store.create_profile(other, "Reviews", "", None).unwrap().id;
        let asset = |workspace, kind| {
            store
                .0
                .query_row(
                    "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size)
                 VALUES (?1, 'Fixture', ?2, 1) RETURNING id",
                    params![workspace, kind],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        };
        let first = asset(studio, "video");
        let second = asset(studio, "video");
        let foreign = asset(other, "video");
        let image = asset(studio, "image");
        let settings = |profile_id, source_ids, intent: &str| DraftSettings {
            title: "  Review  ".into(),
            profile_id,
            intent: intent.into(),
            source_ids,
        };

        for bad in [foreign, image, first] {
            assert!(
                store
                    .create_video_draft(studio, settings(Some(profile), vec![first, bad], ""))
                    .is_err()
            );
        }
        assert!(
            store
                .create_video_draft(studio, settings(Some(foreign_profile), vec![first], ""))
                .is_err()
        );
        assert!(
            store
                .create_video_draft(studio, settings(None, vec![], " "))
                .is_err()
        );
        assert!(store.list_video_drafts(studio).unwrap().is_empty());
        let recorded = store
            .create_video_draft(studio, settings(Some(profile), vec![second, first], ""))
            .unwrap();
        assert_eq!(recorded.settings.title, "Review");
        assert!(
            store
                .update_video_draft(other, recorded.id, settings(None, vec![], "Wrong"))
                .is_err()
        );
        assert!(
            store
                .update_video_draft(
                    studio,
                    recorded.id,
                    settings(None, vec![first, foreign], "Invalid")
                )
                .is_err()
        );
        assert_eq!(store.list_video_drafts(studio).unwrap(), [recorded]);
        let id = store.list_video_drafts(studio).unwrap()[0].id;
        let recorded = store
            .update_video_draft(studio, id, settings(None, vec![first, second], "Revised"))
            .unwrap();
        assert_eq!(recorded.id, id);
        assert_eq!(recorded.settings.source_ids, vec![first, second]);
        let scratch = store
            .create_video_draft(studio, settings(None, vec![], "Explain the cube"))
            .unwrap();
        assert_eq!(
            store.list_video_drafts(studio).unwrap(),
            vec![recorded, scratch]
        );
        assert!(store.list_video_drafts(other).unwrap().is_empty());
    }
}
