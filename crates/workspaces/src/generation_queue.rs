use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::WorkspaceStore;
use crate::format_deliveries::read_settings;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunOptions {
    #[serde(default)]
    pub autopilot: bool,
    pub platform: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GenerationRun {
    pub id: i64,
    pub draft_id: i64,
    pub state: String,
    pub snapshot: Value,
    pub pending_review_id: Option<i64>,
}

impl WorkspaceStore {
    pub fn enqueue_video_draft(
        &self,
        workspace_id: i64,
        draft_id: i64,
        request_key: &str,
        options: &RunOptions,
    ) -> rusqlite::Result<GenerationRun> {
        let key = request_key.trim();
        if key.is_empty() || key.len() > 128 || key.contains('\0') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id, draft_id, state, snapshot_json, pending_review_id FROM generation_runs
             WHERE workspace_id = ?1 AND request_key = ?2",
                params![workspace_id, key],
                read_run,
            )
            .optional()?;
        let options = json!(options);
        let run = if let Some(run) = existing {
            if run.draft_id != draft_id || run.snapshot["options"] != options {
                return Err(rusqlite::Error::InvalidQuery);
            }
            run
        } else {
            let draft = self
                .list_video_drafts(workspace_id)?
                .into_iter()
                .find(|draft| draft.id == draft_id)
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            let profile = match draft.settings.profile_id {
                Some(id) => Some(
                    self.list_profiles(workspace_id)?
                        .into_iter()
                        .find(|profile| profile.id == id)
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
                ),
                None => None,
            };
            let formats = match &profile {
                Some(profile) => self
                    .list_profile_formats(workspace_id, profile.id)?
                    .into_iter()
                    .map(|format| {
                        let delivery =
                            self.profile_delivery(workspace_id, profile.id, format.id)?;
                        Ok(json!({"format": format, "delivery": delivery.effective}))
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()?,
                None => self
                    .list_output_formats(workspace_id)?
                    .into_iter()
                    .filter(|format| format.settings.enabled)
                    .map(|format| {
                        let delivery = self.format_delivery(workspace_id, format.id)?;
                        Ok(json!({"format": format, "delivery": delivery}))
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            };
            if formats.is_empty() {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let snapshot = json!({
                "schema_version": 1,
                "draft": draft,
                "options": options,
                "preview_settings": self.preview_settings()?,
                "workspace_context": self.workspace_context(workspace_id)?,
                "brand_colors": self.brand_colors(workspace_id)?,
                "brand_assets": self.list_brand_assets(workspace_id)?,
                "feedback": self.list_workspace_feedback(workspace_id)?,
                "profile": profile,
                "formats": formats,
            });
            tx.query_row(
                "INSERT INTO generation_runs (workspace_id, draft_id, request_key, snapshot_json)
                 VALUES (?1, ?2, ?3, ?4) RETURNING id, draft_id, state, snapshot_json, pending_review_id",
                params![workspace_id, draft_id, key, snapshot.to_string()],
                read_run,
            )?
        };
        tx.commit()?;
        Ok(run)
    }

    pub fn list_generation_runs(&self, workspace_id: i64) -> rusqlite::Result<Vec<GenerationRun>> {
        self.0
            .prepare(
                "SELECT id, draft_id, state, snapshot_json, pending_review_id FROM generation_runs
             WHERE workspace_id = ?1 ORDER BY id",
            )?
            .query_map([workspace_id], read_run)?
            .collect()
    }
}

pub(crate) fn read_run(row: &Row<'_>) -> rusqlite::Result<GenerationRun> {
    Ok(GenerationRun {
        id: row.get(0)?,
        draft_id: row.get(1)?,
        state: row.get(2)?,
        snapshot: read_settings(row.get(3)?, 3)?,
        pending_review_id: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimensionOverrides, DraftSettings};

    #[test]
    fn queue_freezes_inputs_and_retries_are_idempotent() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let studio = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let profile = store
            .create_profile(studio, "Reviews", "Profile guidance", None)
            .unwrap()
            .id;
        let format = store.list_profile_formats(studio, profile).unwrap()[0].id;
        store
            .set_profile_format_overrides(
                studio,
                profile,
                format,
                &DimensionOverrides {
                    width: Some(1280),
                    height: None,
                },
            )
            .unwrap();
        let draft = store
            .create_video_draft(
                studio,
                DraftSettings {
                    title: "Explainer".into(),
                    profile_id: Some(profile),
                    intent: "Explain the cube".into(),
                    source_ids: vec![],
                },
            )
            .unwrap();
        store
            .set_workspace_context(studio, "Original context")
            .unwrap();
        store.set_preview_max_edge(Some(360)).unwrap();
        let options = RunOptions {
            autopilot: false,
            platform: Some("YouTube".into()),
        };
        assert!(
            store
                .enqueue_video_draft(other, draft.id, "attempt-one", &options)
                .is_err()
        );
        let run = store
            .enqueue_video_draft(studio, draft.id, "attempt-one", &options)
            .unwrap();
        assert_eq!(run.state, "queued");
        assert_eq!(run.snapshot["preview_settings"]["effective_max_edge"], 360);
        assert!(store.set_preview_max_edge(Some(0)).is_err());
        assert_eq!(store.preview_settings().unwrap().effective_max_edge, 360);
        store.set_preview_max_edge(None).unwrap();
        let automatic = store.preview_settings().unwrap();
        assert_eq!(automatic.override_max_edge, None);
        assert_eq!(automatic.effective_max_edge, automatic.recommended_max_edge);
        assert_eq!(run.snapshot["formats"][0]["format"]["width"], 1280);
        assert_eq!(
            run.snapshot["formats"][0]["delivery"]["required"],
            json!(["final_video", "subtitles"])
        );
        store
            .set_workspace_context(studio, "Revised context")
            .unwrap();
        assert_eq!(
            store
                .enqueue_video_draft(studio, draft.id, "attempt-one", &options)
                .unwrap(),
            run
        );
        assert_eq!(run.snapshot["workspace_context"], "Original context");
        assert!(
            store
                .enqueue_video_draft(
                    studio,
                    draft.id,
                    "attempt-one",
                    &RunOptions {
                        autopilot: true,
                        platform: None,
                    }
                )
                .is_err()
        );
        assert_eq!(store.list_generation_runs(studio).unwrap(), [run]);
        assert!(store.list_generation_runs(other).unwrap().is_empty());
    }
}
