use std::collections::BTreeMap;

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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub generate_delivery_subtitles: bool,
    #[serde(default = "default_save_motion_to_library")]
    pub save_motion_to_library: bool,
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provider_overrides: BTreeMap<String, Vec<String>>,
    /// Produce only these output formats for this video; all of them when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<i64>>,
    /// Supplemental settings this video changes for a format, keyed by format ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub format_overrides: BTreeMap<String, FormatOverride>,
    /// Plugin hooks this video does not run, as `plugin-id/action-id`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_hooks: Vec<String>,
}

/// What one video changes about a format's supplemental outputs. Unset fields keep the
/// profile's or workspace's value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnails: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptions: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_guidance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specifications: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait_count: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait_guidance: Option<String>,
    /// Workspace images to use as thumbnail references for this video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_reference_ids: Option<Vec<i64>>,
}

/// Applies a video's output choices to the frozen formats and marks what the video changed.
fn apply_output_overrides(
    formats: Vec<Value>,
    chosen: Option<&[i64]>,
    overrides: &BTreeMap<String, FormatOverride>,
) -> rusqlite::Result<Vec<Value>> {
    let invalid = || rusqlite::Error::InvalidQuery;
    let mut kept = Vec::new();
    for mut entry in formats {
        let id = entry["format"]["id"].as_i64().ok_or_else(invalid)?;
        if chosen.is_some_and(|chosen| !chosen.contains(&id)) {
            continue;
        }
        if let Some(change) = overrides.get(&id.to_string()) {
            let counts = [
                change.thumbnails,
                change.titles,
                change.descriptions,
                change.portrait_count,
            ];
            let texts = [
                &change.thumbnail_guidance,
                &change.specifications,
                &change.portrait_guidance,
            ];
            if counts.iter().flatten().any(|count| *count > 10)
                || texts
                    .iter()
                    .filter_map(|text| text.as_ref())
                    .any(|text| text.len() > 8000)
            {
                return Err(invalid());
            }
            let supplemental = &mut entry["delivery"]["supplemental"];
            let mut changed = Vec::new();
            let mut set = |key: &str, value: Value, supplemental: &mut Value| {
                supplemental[key] = value;
                changed.push(key.to_owned());
            };
            if let Some(value) = change.thumbnails {
                set("thumbnails", json!(value), supplemental);
            }
            if let Some(value) = change.titles {
                set("titles", json!(value), supplemental);
            }
            if let Some(value) = change.descriptions {
                set("descriptions", json!(value), supplemental);
            }
            if let Some(value) = change.tags {
                set("tags", json!(value), supplemental);
            }
            if let Some(value) = &change.thumbnail_guidance {
                set("thumbnail_guidance", json!(value), supplemental);
            }
            if let Some(value) = &change.specifications {
                set("specifications", json!(value), supplemental);
            }
            if let Some(value) = &change.thumbnail_reference_ids {
                if value.len() > 20 {
                    return Err(invalid());
                }
                set("thumbnail_reference_ids", json!(value), supplemental);
            }
            if let Some(value) = change.portrait_count {
                supplemental["portrait_derivatives"]["count"] = json!(value);
                changed.push("portrait_derivatives.count".into());
            }
            if let Some(value) = &change.portrait_guidance {
                if entry["portrait_source"].is_object() {
                    entry["portrait_source"]["guidance"] = json!(value);
                } else {
                    entry["delivery"]["supplemental"]["portrait_derivatives"]["source"]["guidance"] =
                        json!(value);
                }
                changed.push("portrait_derivatives.guidance".into());
            }
            entry["video_overrides"] = json!(changed);
        }
        kept.push(entry);
    }
    if kept.is_empty() {
        return Err(invalid());
    }
    Ok(kept)
}

fn default_save_motion_to_library() -> bool {
    true
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            autopilot: false,
            generate_delivery_subtitles: false,
            save_motion_to_library: default_save_motion_to_library(),
            platform: None,
            provider_overrides: BTreeMap::new(),
            formats: None,
            format_overrides: BTreeMap::new(),
            skip_hooks: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GenerationRun {
    pub id: i64,
    pub draft_id: i64,
    pub state: String,
    pub snapshot: Value,
    pub pending_review_id: Option<i64>,
    pub display_title: Option<String>,
    /// Present in run listings so overviews can show where each run is without loading its plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<RunProgress>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct RunProgress {
    pub stage_index: i64,
    pub stage_count: i64,
    pub stage: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    Captured,
    Current,
}

impl ReplayMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::Current => "current",
        }
    }
}

type ReplayOrigin = (i64, String);

fn generation_request_key(request_key: &str) -> rusqlite::Result<&str> {
    let key = request_key.trim();
    if key.is_empty() || key.len() > 128 || key.contains('\0') {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(key)
}

#[derive(Debug, Serialize)]
pub struct PendingReview {
    pub run_id: i64,
    pub title: String,
}

impl WorkspaceStore {
    pub fn enqueue_video_draft(
        &self,
        workspace_id: i64,
        draft_id: i64,
        request_key: &str,
        options: &RunOptions,
    ) -> rusqlite::Result<GenerationRun> {
        let key = generation_request_key(request_key)?;
        if options.skip_hooks.len() > 64
            || !options.skip_hooks.iter().all(|hook| {
                hook.split_once('/').is_some_and(|(plugin, action)| {
                    crate::plugin_catalog::identifier(plugin)
                        && crate::plugin_catalog::identifier(action)
                })
            })
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let options = json!(options);
        let run = if let Some((run, replay)) = self.generation_request(workspace_id, key)? {
            if replay.is_some() || run.draft_id != draft_id || run.snapshot["options"] != options {
                return Err(rusqlite::Error::InvalidQuery);
            }
            run
        } else {
            let snapshot = self.capture_generation_inputs(workspace_id, draft_id, &options)?;
            self.insert_generation_run(workspace_id, draft_id, key, &snapshot, None)?
        };
        tx.commit()?;
        Ok(run)
    }

    pub fn replay_generation(
        &self,
        workspace_id: i64,
        source_run_id: i64,
        request_key: &str,
        mode: ReplayMode,
    ) -> rusqlite::Result<GenerationRun> {
        let key = generation_request_key(request_key)?;
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let source = self.0.query_row(
            "SELECT id, draft_id, state, snapshot_json, pending_review_id, display_title FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'",
            params![workspace_id, source_run_id],
            read_run,
        )?;
        let replay = Some((source_run_id, mode.as_str().to_owned()));
        let run =
            if let Some((run, existing_replay)) = self.generation_request(workspace_id, key)? {
                if existing_replay != replay {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                run
            } else {
                let snapshot = match mode {
                    ReplayMode::Captured => source.snapshot,
                    ReplayMode::Current => self.capture_generation_inputs(
                        workspace_id,
                        source.draft_id,
                        &source.snapshot["options"],
                    )?,
                };
                self.insert_generation_run(workspace_id, source.draft_id, key, &snapshot, replay)?
            };
        tx.commit()?;
        Ok(run)
    }

    fn generation_request(
        &self,
        workspace_id: i64,
        key: &str,
    ) -> rusqlite::Result<Option<(GenerationRun, Option<ReplayOrigin>)>> {
        self.0.query_row(
            "SELECT id, draft_id, state, snapshot_json, pending_review_id, replay_of_run_id, replay_mode, display_title
             FROM generation_runs WHERE workspace_id = ?1 AND request_key = ?2",
            params![workspace_id, key],
            |row| {
                let source = row.get::<_, Option<i64>>(5)?;
                let replay = source.map(|id| row.get::<_, String>(6).map(|mode| (id, mode))).transpose()?;
                Ok((read_run(row)?, replay))
            },
        ).optional()
    }

    fn insert_generation_run(
        &self,
        workspace_id: i64,
        draft_id: i64,
        key: &str,
        snapshot: &Value,
        replay: Option<ReplayOrigin>,
    ) -> rusqlite::Result<GenerationRun> {
        let (source, mode) = replay.unzip();
        let snapshot = snapshot.to_string();
        let plan = crate::generation_plan::plan_snapshot(&snapshot)?;
        let plan_json = serde_json::to_string(&plan)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "INSERT INTO generation_runs (workspace_id, draft_id, request_key, snapshot_json, replay_of_run_id, replay_mode, plan_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id, draft_id, state, snapshot_json, pending_review_id, display_title",
            params![workspace_id, draft_id, key, snapshot, source, mode, plan_json],
            read_run,
        )
    }

    /// When a format's portrait cuts follow another profile, copies that profile's guidance and its
    /// portrait format's thumbnail settings into the run, so cuts are made the way that profile
    /// makes videos. A profile deleted since falls back to unguided cuts.
    fn resolve_portrait_source(
        &self,
        workspace_id: i64,
        entry: &mut Value,
    ) -> rusqlite::Result<()> {
        let source = &entry["delivery"]["supplemental"]["portrait_derivatives"]["source"];
        if source["type"] != "profile" {
            return Ok(());
        }
        let Some(profile_id) = source["profile_id"].as_i64() else {
            return Ok(());
        };
        let profile = self
            .list_profiles(workspace_id)?
            .into_iter()
            .find(|profile| profile.id == profile_id);
        let mut resolved = json!({"type": "profile", "profile_id": profile_id});
        if let Some(profile) = profile {
            let formats = self.list_profile_formats(workspace_id, profile.id)?;
            let portrait = formats
                .iter()
                .find(|format| format.height > format.width)
                .or(formats.first());
            let mut guidance = vec![profile.guidance.trim().to_owned()];
            resolved["profile_name"] = json!(profile.name);
            if let Some(format) = portrait {
                let supplemental = self
                    .profile_delivery(workspace_id, profile.id, format.id)?
                    .effective
                    .supplemental;
                guidance.push(supplemental.specifications.trim().to_owned());
                resolved["thumbnails_per_video"] = json!(supplemental.thumbnails.min(3));
                resolved["thumbnail_guidance"] = json!(supplemental.thumbnail_guidance);
                resolved["thumbnail_reference_ids"] = json!(supplemental.thumbnail_reference_ids);
            }
            guidance.retain(|part| !part.is_empty());
            resolved["guidance"] = json!(guidance.join("\n\n"));
        }
        // Kept beside the stored setting, which run planning reads with its strict shape.
        entry["portrait_source"] = resolved;
        Ok(())
    }

    fn capture_generation_inputs(
        &self,
        workspace_id: i64,
        draft_id: i64,
        options: &Value,
    ) -> rusqlite::Result<Value> {
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
                    let delivery = self.profile_delivery(workspace_id, profile.id, format.id)?;
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
        let mut formats = formats;
        for entry in &mut formats {
            self.resolve_portrait_source(workspace_id, entry)?;
        }
        let chosen: Option<Vec<i64>> = options
            .get("formats")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let format_overrides: BTreeMap<String, FormatOverride> = options
            .get("format_overrides")
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
            .unwrap_or_default();
        let references: Vec<i64> = format_overrides
            .values()
            .filter_map(|change| change.thumbnail_reference_ids.as_ref())
            .flatten()
            .copied()
            .collect();
        for asset in references {
            let usable: bool = self.0.query_row(
                "SELECT EXISTS (SELECT 1 FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2
                 AND kind = 'image' AND superseded = 0)",
                rusqlite::params![workspace_id, asset],
                |row| row.get(0),
            )?;
            if !usable {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        let formats = apply_output_overrides(formats, chosen.as_deref(), &format_overrides)?;
        let overrides = options
            .get("provider_overrides")
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
            .unwrap_or_default();
        let provider_orders = self
            .capture_provider_orders(workspace_id, &overrides)
            .map_err(|error| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    error.to_string(),
                )))
            })?;
        let motion_renderer = provider_orders
            .get("motion-render")
            .and_then(|order| order.effective.first())
            .map(|id| self.capture_motion_renderer(id))
            .transpose()?
            .flatten();
        let agent_settings = self.agent_settings(Some(workspace_id))?;
        // Freeze the agent's plugin settings so the run reproduces with the same capabilities.
        let agent_options = agent_settings
            .effective_provider
            .as_deref()
            .map(|provider| self.agent_options(workspace_id, provider))
            .unwrap_or_default();
        Ok(json!({
            "schema_version": 1,
            "motion_renderer": motion_renderer,
            "agent_options": agent_options,
            "draft": draft,
            "options": options,
            "preview_settings": self.preview_settings()?,
            "agent_settings": agent_settings,
            "provider_orders": provider_orders,
            "workspace_context": self.workspace_context(workspace_id)?,
            "brand_colors": self.video_brand_colors(workspace_id)?,
            "brand_assets": self.list_brand_assets(workspace_id)?,
            "feedback": self.list_workspace_feedback(workspace_id)?,
            "delivery_feedback": self.draft_delivery_feedback(workspace_id, draft_id)?,
            "profile_assets": match &profile {
                Some(profile) => json!(self.list_profile_assets(workspace_id, profile.id)?),
                None => json!([]),
            },
            "profile": profile,
            "formats": formats,
        }))
    }

    pub fn pending_generation_reviews(
        &self,
        workspace_id: i64,
    ) -> rusqlite::Result<Vec<PendingReview>> {
        self.0
            .prepare(
                "SELECT id, json_extract(snapshot_json, '$.draft.title') FROM generation_runs
             WHERE workspace_id = ?1 AND state = 'paused' AND pending_review_id IS NOT NULL
             ORDER BY id DESC",
            )?
            .query_map([workspace_id], |row| {
                Ok(PendingReview {
                    run_id: row.get(0)?,
                    title: row.get(1)?,
                })
            })?
            .collect()
    }

    pub fn generation_run(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Option<GenerationRun>> {
        self.0
            .query_row(
                &format!("{RUN_WITH_PROGRESS} WHERE workspace_id = ?1 AND id = ?2"),
                params![workspace_id, run_id],
                read_run_with_progress,
            )
            .optional()
    }

    pub fn list_generation_runs(&self, workspace_id: i64) -> rusqlite::Result<Vec<GenerationRun>> {
        self.0
            .prepare(&format!(
                "{RUN_WITH_PROGRESS} WHERE workspace_id = ?1 ORDER BY id"
            ))?
            .query_map([workspace_id], read_run_with_progress)?
            .collect()
    }
}

const RUN_WITH_PROGRESS: &str =
    "SELECT id, draft_id, state, snapshot_json, pending_review_id, display_title,
    stage_index, json_array_length(plan_json, '$.steps'),
    json_extract(plan_json, '$.steps[' || stage_index || '].stage')
    FROM generation_runs";

fn read_run_with_progress(row: &Row<'_>) -> rusqlite::Result<GenerationRun> {
    let mut run = read_run(row)?;
    run.progress = row
        .get::<_, Option<i64>>(7)?
        .map(|stage_count| -> rusqlite::Result<_> {
            Ok(RunProgress {
                stage_index: row.get(6)?,
                stage_count,
                stage: row.get(8)?,
            })
        })
        .transpose()?;
    Ok(run)
}

pub(crate) fn read_run(row: &Row<'_>) -> rusqlite::Result<GenerationRun> {
    Ok(GenerationRun {
        id: row.get(0)?,
        draft_id: row.get(1)?,
        state: row.get(2)?,
        snapshot: read_settings(row.get(3)?, 3)?,
        pending_review_id: row.get(4)?,
        display_title: row.get("display_title")?,
        progress: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimensionOverrides, DraftSettings};

    #[test]
    fn portrait_cuts_follow_their_source_profile() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let studio = store.create("Studio").unwrap().id;
        let shorts = store
            .create_profile(studio, "Shorts", "Hook in the first second.", None)
            .unwrap()
            .id;
        let formats = store.list_output_formats(studio).unwrap();
        let landscape = formats
            .iter()
            .find(|f| f.settings.width > f.settings.height)
            .unwrap()
            .id;
        let portrait = formats
            .iter()
            .find(|f| f.settings.height > f.settings.width)
            .unwrap()
            .id;
        store
            .set_format_delivery(
                studio,
                portrait,
                &crate::SupplementalOutputs {
                    thumbnails: 2,
                    thumbnail_guidance: "Big face, one word".into(),
                    specifications: "Under 45 seconds.".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        store
            .set_format_delivery(
                studio,
                landscape,
                &crate::SupplementalOutputs {
                    portrait_derivatives: crate::PortraitDerivatives {
                        count: 2,
                        source: crate::PortraitSource::Profile { profile_id: shorts },
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        let draft = store
            .create_video_draft(
                studio,
                DraftSettings {
                    title: "Explainer".into(),
                    profile_id: None,
                    intent: "Explain the cube".into(),
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(studio, draft.id, "portrait-source", &RunOptions::default())
            .unwrap();
        let entry = run.snapshot["formats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["format"]["id"] == landscape)
            .unwrap();
        assert_eq!(
            entry["delivery"]["supplemental"]["portrait_derivatives"]["source"],
            json!({"type": "profile", "profile_id": shorts})
        );
        let source = &entry["portrait_source"];
        assert_eq!(source["profile_name"], "Shorts");
        assert_eq!(
            source["guidance"],
            "Hook in the first second.\n\nUnder 45 seconds."
        );
        assert_eq!(source["thumbnails_per_video"], 2);
        assert_eq!(source["thumbnail_guidance"], "Big face, one word");
        let request = &crate::portrait_cuts::cut_requests(&run.snapshot)[&landscape];
        assert_eq!(request.thumbnails_per_cut, 2);
        assert!(request.guidance.contains("Shorts"), "{}", request.guidance);

        let customised = store
            .enqueue_video_draft(
                studio,
                draft.id,
                "portrait-guidance",
                &RunOptions {
                    format_overrides: BTreeMap::from([(
                        landscape.to_string(),
                        FormatOverride {
                            portrait_guidance: Some("Only the reveal.".into()),
                            ..FormatOverride::default()
                        },
                    )]),
                    ..RunOptions::default()
                },
            )
            .unwrap();
        let request = &crate::portrait_cuts::cut_requests(&customised.snapshot)[&landscape];
        assert!(
            request.guidance.ends_with("Only the reveal."),
            "{}",
            request.guidance
        );
        let foreign = RunOptions {
            format_overrides: BTreeMap::from([(
                landscape.to_string(),
                FormatOverride {
                    thumbnail_reference_ids: Some(vec![999]),
                    ..FormatOverride::default()
                },
            )]),
            ..RunOptions::default()
        };
        assert!(
            store
                .enqueue_video_draft(studio, draft.id, "foreign-reference", &foreign)
                .is_err()
        );
        assert_eq!(request.thumbnails_per_cut, 2);
    }

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
            ..RunOptions::default()
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
        let default_order = store
            .provider_preferences("transcription", Some(studio))
            .unwrap()
            .effective;
        let captured = &run.snapshot["provider_orders"]["transcription"];
        if !default_order.is_empty() {
            assert_eq!(
                captured,
                &json!({"effective": default_order, "source": "bundled_default"})
            );
        }
        store
            .set_provider_preferences("transcription", Some(studio), Some(&[]))
            .unwrap();

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
                        ..RunOptions::default()
                    }
                )
                .is_err()
        );
        let mut listed = store.list_generation_runs(studio).unwrap();
        let steps = store
            .generation_plan(studio, run.id)
            .unwrap()
            .unwrap()
            .steps
            .len() as i64;
        assert_eq!(
            listed[0].progress.take(),
            Some(RunProgress {
                stage_index: 0,
                stage_count: steps,
                stage: Some("script".into())
            })
        );
        assert_eq!(listed, [run]);
        assert!(store.list_generation_runs(other).unwrap().is_empty());
        let revised = store
            .enqueue_video_draft(studio, draft.id, "with-override", &options)
            .unwrap();
        assert_eq!(
            revised.snapshot["provider_orders"]["transcription"],
            json!({"effective": [], "source": "workspace"})
        );
        store.install_plugin_manifest(br#"{
            "schemaVersion": 1,
            "id": "fixture-provider",
            "name": "Fixture Provider",
            "description": "Test fixture",
            "capabilities": {"transcription": {"name": "Transcription", "description": "Test fixture"}}
        }"#).unwrap();
        store.set_plugin_active("fixture-provider", true).unwrap();
        let override_options = RunOptions {
            provider_overrides: BTreeMap::from([(
                "transcription".into(),
                vec!["fixture-provider".into()],
            )]),
            ..options
        };
        let overridden = store
            .enqueue_video_draft(studio, draft.id, "per-run", &override_options)
            .unwrap();
        assert_eq!(
            overridden.snapshot["provider_orders"]["transcription"],
            json!({"effective": ["fixture-provider"], "source": "run"})
        );
        assert!(
            store
                .enqueue_video_draft(studio, draft.id, "attempt-one", &override_options)
                .is_err()
        );
    }

    #[test]
    fn videos_choose_formats_and_override_their_outputs() {
        let formats = vec![
            json!({"format": {"id": 1}, "delivery": {"supplemental": {"thumbnails": 3, "titles": 3, "thumbnail_guidance": "A/B",
                "portrait_derivatives": {"count": 3, "source": {"type": "manual", "guidance": ""}}}}}),
            json!({"format": {"id": 2}, "delivery": {"supplemental": {"thumbnails": 0}}}),
        ];
        let overrides = BTreeMap::from([(
            "1".to_owned(),
            FormatOverride {
                thumbnails: Some(1),
                thumbnail_guidance: Some("Show the cube big".into()),
                portrait_count: Some(0),
                ..FormatOverride::default()
            },
        )]);
        let kept = apply_output_overrides(formats.clone(), Some(&[1]), &overrides).unwrap();
        assert_eq!(kept.len(), 1, "only the chosen format is produced");
        let supplemental = &kept[0]["delivery"]["supplemental"];
        assert_eq!(supplemental["thumbnails"], 1);
        assert_eq!(
            supplemental["titles"], 3,
            "unset fields keep the profile's value"
        );
        assert_eq!(supplemental["thumbnail_guidance"], "Show the cube big");
        assert_eq!(supplemental["portrait_derivatives"]["count"], 0);
        assert_eq!(
            kept[0]["video_overrides"],
            json!([
                "thumbnails",
                "thumbnail_guidance",
                "portrait_derivatives.count"
            ])
        );
        assert!(
            apply_output_overrides(formats.clone(), Some(&[9]), &BTreeMap::new()).is_err(),
            "a video produces something"
        );
        let too_many = BTreeMap::from([(
            "1".to_owned(),
            FormatOverride {
                titles: Some(50),
                ..FormatOverride::default()
            },
        )]);
        assert!(apply_output_overrides(formats, None, &too_many).is_err());
    }
}
