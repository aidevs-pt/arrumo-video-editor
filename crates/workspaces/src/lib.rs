use std::{path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

mod a2a;
mod agent_cli;
mod lease_keeper;
mod quiet_process;
mod revision_media;
pub(crate) use quiet_process::quiet_command;
mod agent_explore;
mod agent_images;
mod agent_logs;
mod agent_plugins;
mod agent_run;
mod agent_sessions;
mod agent_settings;
mod agent_stage;
mod agent_work;
mod asset_cleanup;
mod asset_descriptions;
pub use asset_cleanup::{AssetRemoval, AssetUse, AssetUseKind};
pub use asset_descriptions::{DescribedAssets, DescriptionPlan, OmitReason, OmittedAsset};
mod asset_organization;
mod asset_plan;
mod assets;
mod audio_generation;
mod automatic_subtitles;
mod backup;
mod brand_assets;
mod brand_fonts;
mod clip_render;
mod completed_video;
mod completed_video_feedback;
mod context_import;
mod context_research;
mod database_snapshot;
mod delivery_captions;
mod delivery_cleanup;
mod delivery_export;
mod delivery_verification;
mod format_deliveries;
mod generation_notifications;
mod generation_plan;
mod generation_queue;
mod generation_worker;
mod image_generation;
pub use image_generation::ASSET_ASPECT_RATIOS;
mod mcp;
mod media_tools;
pub use media_tools::{MediaToolsStatus, media_tools_status};
mod media_worker;
mod motion_artifacts;
mod motion_design;
mod motion_library;
mod motion_renderer;
mod motion_scratch;
mod motion_spec;
mod narration;
mod operations;
mod output_formats;
mod output_schedule;
mod plugin_action_jobs;
mod plugin_actions;
mod plugin_activation;
pub use plugin_activation::plugin_activation_refusal_code;
mod audio_transform;
mod caption_style;
mod dubbing;
mod editing_guides;
mod frame_strip;
mod join_review;
mod media_order_limits;
mod plugin_catalog;
mod plugin_credentials;
mod plugin_grants;
mod plugin_hooks;
mod plugin_package;
mod plugin_registry;
mod plugin_runtime;
mod plugin_schema;
mod plugin_settings;
mod plugin_surfaces;
mod plugin_text;
mod plugin_tour;
mod portrait_cuts;
mod preview_edit;
mod preview_review;
mod preview_settings;
pub use media_order_limits::MediaOrderLimits;
mod production_progress;
mod profile_deliveries;
mod profile_drafting;
mod profile_formats;
mod profile_organization;
mod provider_jobs;
mod provider_media;
mod provider_preferences;
mod provider_usage;
mod scene_transitions;
mod script_stage;
pub(crate) mod source_pages;
mod source_transcripts;
mod speech_synthesis;
mod subtitle_revisions;
mod supplemental_delivery;
mod supplemental_review;
mod supplemental_writing;
mod telemetry;
mod transcription;
mod video_drafts;
mod video_generation;
pub use image_generation::ImageModel;
pub use provider_media::Localized;
pub use provider_usage::{ProviderUsage, USAGE_DANGER_PERCENT, USAGE_WARNING_PERCENT};
pub use speech_synthesis::SpeechModel;
pub use video_generation::{VideoModel, VideoVariant};
mod workspace_feedback;

pub use a2a::serve_a2a;
pub use agent_cli::{
    AgentExecutableError, AgentLoginStatus, agent_cli_detected, agent_cli_path, check_agent_login,
};
pub use agent_logs::{AgentLog, AgentLogSummary};
pub use agent_plugins::{AgentPlugin, agent_plugin, agent_plugins};
pub use agent_run::AgentRunError;
pub use agent_settings::{AgentProviderSource, AgentSettings};
pub use agent_work::agent_work_root;
pub use asset_organization::AssetOrganization;
pub use assets::{
    ImagePreview, WorkspaceAsset, asset_path, asset_root, frame_cache, image_type, is_canonical,
};
pub use audio_generation::AudioKind;
pub use audio_transform::{AudioTransform, AudioTransformProvider};
pub use backup::{
    BackupManifest, BackupSummary, apply_pending_import, cancel_backup_import,
    pending_backup_import, stage_backup_import, verify_backup,
};
pub use brand_assets::{BrandAsset, BrandAssetRole};
pub use brand_fonts::{BrandFont, FontRole, FontWeight, UnusableFont};
pub use caption_style::{
    CaptionFont, CaptionPosition, CaptionStyle, CaptionStyleOverrides, caption_preview,
};
pub use completed_video::CompletedVideo;
pub use completed_video_feedback::CompletedVideoFeedback;
pub use context_import::{ContextImport, ContextImportOutcome};
pub use context_research::{
    ContextDraft, ContextSource, SuggestedImage, SuggestedProfile, UnreadSource,
};
pub use database_snapshot::DatabaseSnapshot;
pub use delivery_cleanup::DeliveryCleanupReport;
pub use delivery_verification::{VerifiedDelivery, VerifiedOutput, delivery_directory};
pub use dubbing::DubbingProvider;
pub use editing_guides::{EDITING_GUIDES, EditingGuide, editing_guide};
pub use format_deliveries::{
    FormatDelivery, PortraitDerivatives, PortraitSource, RequiredOutput, SupplementalOutputs,
};
pub use frame_strip::{FrameStrip, FrameStripSource, render_frame_strip};
pub use generation_notifications::{GenerationInbox, GenerationNotification, SystemNotice};
pub use generation_plan::{GenerationPlan, PlanStep, ProductionStage};
pub use generation_queue::{
    FormatOverride, GenerationRun, PendingReview, ReplayMode, RunOptions, RunProgress,
};
pub use generation_worker::{GenerationActivity, GenerationClaim, RunControl};
pub use mcp::serve_mcp;
pub use media_worker::MediaWork;
pub use motion_artifacts::MotionArtifact;
pub use motion_library::{MotionKind, MotionLibraryItem, MotionLibrarySettings};
pub use motion_renderer::{MotionWorkerPaths, configured_motion_worker};
pub use motion_scratch::recover_motion_scratch;
pub use output_formats::{OutputFormat, OutputFormatSettings};
pub use output_schedule::ScheduledOutput;
pub use plugin_action_jobs::{PluginActionJob, PluginActionJobRequest};
pub use plugin_actions::InstalledPluginAction;
pub use plugin_catalog::{
    CatalogPlugin, MAX_PLUGIN_MANIFEST_BYTES, PluginManifest, SUPPORTED_PLUGIN_MANIFEST_SCHEMAS,
};
pub use plugin_credentials::{MAX_PLUGIN_SECRET_BYTES, PluginCredentialStatus};
pub use plugin_grants::PluginContextRequest;
pub use plugin_package::{PluginPackageInfo, THIRD_PARTY_PLUGIN_NOTICE};
pub use plugin_registry::{
    PluginRegistry, RegistryPlugin, RegistryRelease, RegistryReleaseSource, fetch_plugin_registry,
    read_plugin_registry, validate_registry_url,
};
pub use plugin_runtime::MAX_PLUGIN_IO_BYTES;
pub use plugin_schema::{
    ActionHook, ActionInput, ActionInputKind, ActionPlacement, HookEvent, PluginAction,
    PluginActionScope, PluginCode, PluginSetting, PluginSettingKind, PluginSettingScope,
    PluginSurfacePlacement,
};
pub use plugin_settings::ResolvedPluginSetting;
pub use plugin_surfaces::InstalledPluginSurface;
pub use plugin_tour::TourStop;
pub use preview_review::{PreviewFormatDecision, PreviewReplacement, PreviewRevisionRequest};
pub use preview_settings::PreviewSettings;
pub use production_progress::{ProductionProgress, ProductionResult, ReviewDecision};
pub use profile_deliveries::{DeliveryOverrides, ProfileDelivery};
pub use profile_drafting::ProfileDraft;
pub use profile_formats::{DimensionOverrides, ProfileFormat};
pub use profile_organization::{AssetProfileShare, MAX_PROFILE_DESCRIPTION, ProfileAsset};
pub use provider_preferences::{CapabilityProvider, ProviderPreferences};
pub use source_pages::SourceProblem;
pub use speech_synthesis::SpeechProviderStatus;
pub use subtitle_revisions::{SubtitleRevision, read_subtitle_file};
pub use supplemental_delivery::{SupplementalMedia, SupplementalText};
pub use supplemental_review::{
    RequestedReplacement, SupplementalDecision, SupplementalItemNotes, SupplementalReview,
    SupplementalRevisionRequest,
};
pub use telemetry::{
    PrivacySettings, allow_crash_reports, crash_dsn, crash_reports_allowed, error_chain,
    install_crash_reporter, redact_report_text, report_failure, report_failure_with,
    set_failure_sink, set_usage_sink, usage_key,
};
pub use transcription::{ProviderTranscript, Transcript, TranscriptWord};
pub use video_drafts::{DraftSettings, VideoDraft};
pub use workspace_feedback::WorkspaceFeedback;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    /// `#RRGGBB` behind the icon.
    pub color: Option<String>,
    /// One of [`WORKSPACE_ICONS`].
    pub icon: Option<String>,
    /// A workspace image shown instead of the icon.
    pub image_asset_id: Option<i64>,
}

/// Icons a workspace can be shown with. The desktop draws each with the matching Lucide icon.
pub const WORKSPACE_ICONS: [&str; 16] = [
    "clapperboard",
    "camera",
    "mic",
    "sparkles",
    "rocket",
    "gamepad",
    "book",
    "palette",
    "music",
    "code",
    "heart",
    "leaf",
    "briefcase",
    "graduation-cap",
    "utensils",
    "dumbbell",
];
const WORKSPACE_COLORS: [&str; 8] = [
    "#E86AA8", "#4BA3F2", "#3DBB6B", "#E55D4D", "#C68820", "#138778", "#6974D6", "#AB5980",
];

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub guidance: String,
    pub color: Option<String>,
    pub description: String,
}

/// One brand colour: `#RRGGBB`, an optional name, and whether videos use it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrandColor {
    pub hex: String,
    pub name: String,
    pub for_videos: bool,
}

pub struct WorkspaceStore(Connection);

impl WorkspaceStore {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let migration = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = migration.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let migrations = [
            include_str!("../migrations/001-create-workspaces.sql"),
            include_str!("../migrations/002-create-profiles.sql"),
            include_str!("../migrations/003-create-output-formats.sql"),
            include_str!("../migrations/004-profile-format-overrides.sql"),
            include_str!("../migrations/005-format-deliveries.sql"),
            include_str!("../migrations/006-profile-deliveries.sql"),
            include_str!("../migrations/007-workspace-assets.sql"),
            include_str!("../migrations/008-workspace-context.sql"),
            include_str!("../migrations/009-workspace-feedback.sql"),
            include_str!("../migrations/010-workspace-brand.sql"),
            include_str!("../migrations/011-brand-assets.sql"),
            include_str!("../migrations/012-video-drafts.sql"),
            include_str!("../migrations/013-generation-queue.sql"),
            include_str!("../migrations/014-generation-activity.sql"),
            include_str!("../migrations/015-generation-failures.sql"),
            include_str!("../migrations/016-generation-plan.sql"),
            include_str!("../migrations/017-production-progress.sql"),
            include_str!("../migrations/018-preview-settings.sql"),
            include_str!("../migrations/019-completed-video-title.sql"),
            include_str!("../migrations/020-generation-replay.sql"),
            include_str!("../migrations/021-completed-video-feedback.sql"),
            include_str!("../migrations/022-agent-settings.sql"),
            include_str!("../migrations/023-agent-plugin-ids.sql"),
            include_str!("../migrations/024-plugin-catalog.sql"),
            include_str!("../migrations/025-plugin-packages.sql"),
            include_str!("../migrations/026-plugin-release-source.sql"),
            include_str!("../migrations/027-plugin-settings.sql"),
            include_str!("../migrations/028-credential-namespace.sql"),
            include_str!("../migrations/029-provider-preferences.sql"),
            include_str!("../migrations/030-plugin-grants.sql"),
            include_str!("../migrations/031-generation-notifications.sql"),
            include_str!("../migrations/032-generation-notification-reads.sql"),
            include_str!("../migrations/033-delivery-cleanup.sql"),
            include_str!("../migrations/034-output-schedules.sql"),
            include_str!("../migrations/035-asset-organization.sql"),
            include_str!("../migrations/036-asset-cleanup.sql"),
            include_str!("../migrations/037-asset-replacements.sql"),
            include_str!("../migrations/038-motion-library.sql"),
            include_str!("../migrations/039-motion-profile-sharing.sql"),
            include_str!("../migrations/040-motion-result-origin.sql"),
            include_str!("../migrations/041-subtitle-revisions.sql"),
            include_str!("../migrations/042-supplemental-review.sql"),
            include_str!("../migrations/043-preview-review.sql"),
            include_str!("../migrations/044-subtitle-languages.sql"),
            include_str!("../migrations/045-bundled-plugin-grants.sql"),
            include_str!("../migrations/046-completed-media.sql"),
            include_str!("../migrations/047-stable-completed-media.sql"),
            include_str!("../migrations/048-variant-content-identity.sql"),
            include_str!("../migrations/049-plugin-action-jobs.sql"),
            include_str!("../migrations/050-automatic-delivery-subtitles.sql"),
            include_str!("../migrations/051-profile-organization.sql"),
            include_str!("../migrations/052-source-transcripts.sql"),
            include_str!("../migrations/053-agent-logs.sql"),
            include_str!("../migrations/054-motion-specs.sql"),
            include_str!("../migrations/055-generated-assets.sql"),
            include_str!("../migrations/056-footage-assets.sql"),
            include_str!("../migrations/057-privacy-settings.sql"),
            include_str!("../migrations/058-plugin-tour.sql"),
            include_str!("../migrations/059-agent-sessions.sql"),
            include_str!("../migrations/060-workspace-appearance.sql"),
            include_str!("../migrations/061-context-sources.sql"),
            include_str!("../migrations/062-brand-color-details.sql"),
            include_str!("../migrations/063-system-notifications.sql"),
            include_str!("../migrations/064-plugin-activation.sql"),
            include_str!("../migrations/065-agent-usage-observations.sql"),
            include_str!("../migrations/066-brand-fonts.sql"),
            include_str!("../migrations/067-agent-executables.sql"),
            include_str!("../migrations/068-plugin-tour-for-new-installs.sql"),
            include_str!("../migrations/069-install-event.sql"),
            include_str!("../migrations/070-media-order-limits.sql"),
            include_str!("../migrations/071-agent-rounds.sql"),
        ];
        let pending = usize::try_from(version)
            .ok()
            .and_then(|version| migrations.get(version..))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        for migration_sql in pending {
            migration.execute_batch(migration_sql)?;
        }
        if version == 0 {
            migration.execute("UPDATE privacy_settings SET install_reported = 0", [])?;
        }
        migration.commit()?;
        agent_cli::load_executables(&connection)?;
        Ok(Self(connection))
    }

    pub fn create(&self, name: &str) -> rusqlite::Result<Workspace> {
        let name = name.trim();
        let seed = uuid::Uuid::new_v4().into_bytes();
        let color = WORKSPACE_COLORS[usize::from(seed[0]) % WORKSPACE_COLORS.len()];
        let icon = WORKSPACE_ICONS[usize::from(seed[1]) % 8];
        self.0
            .query_row(
                "INSERT INTO workspaces (name, color, icon) VALUES (?1, ?2, ?3)
             RETURNING id, name, color, icon, image_asset_id",
                rusqlite::params![name, color, icon],
                Self::read_workspace,
            )
            .inspect(|_| self.track_usage("workspace_created", &[]))
    }

    /// Sets how the workspace is shown: a color, a preset icon, and optionally one of its
    /// images, which then replaces the icon.
    pub fn set_workspace_appearance(
        &self,
        workspace_id: i64,
        color: &str,
        icon: &str,
        image_asset_id: Option<i64>,
    ) -> rusqlite::Result<Workspace> {
        let hex = color
            .strip_prefix('#')
            .filter(|value| {
                value.len() == 6 && value.bytes().all(|digit| digit.is_ascii_hexdigit())
            })
            .ok_or(rusqlite::Error::InvalidQuery)?;
        if !WORKSPACE_ICONS.contains(&icon) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(asset) = image_asset_id {
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
        self.0.query_row(
            "UPDATE workspaces SET color = ?2, icon = ?3, image_asset_id = ?4 WHERE id = ?1
             RETURNING id, name, color, icon, image_asset_id",
            rusqlite::params![
                workspace_id,
                format!("#{}", hex.to_ascii_uppercase()),
                icon,
                image_asset_id
            ],
            Self::read_workspace,
        )
    }

    /// Renames a workspace.
    pub fn rename_workspace(&self, workspace_id: i64, name: &str) -> rusqlite::Result<Workspace> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "UPDATE workspaces SET name = ?2 WHERE id = ?1 RETURNING id, name, color, icon, image_asset_id",
            rusqlite::params![workspace_id, name],
            Self::read_workspace,
        )
    }

    pub fn list(&self) -> rusqlite::Result<Vec<Workspace>> {
        self.0
            .prepare("SELECT id, name, color, icon, image_asset_id FROM workspaces ORDER BY id")?
            .query_map([], Self::read_workspace)?
            .collect()
    }

    pub fn workspace_context(&self, workspace_id: i64) -> rusqlite::Result<Option<String>> {
        self.0.query_row(
            "SELECT context_markdown FROM workspaces WHERE id = ?1",
            [workspace_id],
            |row| row.get(0),
        )
    }

    pub fn set_workspace_context(&self, workspace_id: i64, markdown: &str) -> rusqlite::Result<()> {
        self.0.query_row(
            "UPDATE workspaces SET context_markdown = ?2 WHERE id = ?1 RETURNING id",
            rusqlite::params![workspace_id, markdown],
            |_| Ok(()),
        )
    }

    pub fn brand_colors(&self, workspace_id: i64) -> rusqlite::Result<Vec<String>> {
        let json = self.0.query_row(
            "SELECT brand_colors_json FROM workspaces WHERE id = ?1",
            [workspace_id],
            |row| row.get::<_, String>(0),
        )?;
        format_deliveries::read_settings(Some(json), 0)
    }

    /// The brand colours videos use: every colour except those the creator turned off.
    pub fn video_brand_colors(&self, workspace_id: i64) -> rusqlite::Result<Vec<String>> {
        Ok(self
            .brand_palette(workspace_id)?
            .into_iter()
            .filter(|color| color.for_videos)
            .map(|color| color.hex)
            .collect())
    }

    /// Every brand colour with its name and whether videos use it, in palette order.
    pub fn brand_palette(&self, workspace_id: i64) -> rusqlite::Result<Vec<BrandColor>> {
        let details: serde_json::Map<String, serde_json::Value> = self
            .0
            .query_row(
                "SELECT brand_color_details_json FROM workspaces WHERE id = ?1",
                [workspace_id],
                |row| row.get::<_, String>(0),
            )
            .map(|json| serde_json::from_str(&json).unwrap_or_default())?;
        Ok(self
            .brand_colors(workspace_id)?
            .into_iter()
            .map(|hex| {
                let detail = details.get(&hex);
                BrandColor {
                    name: detail
                        .and_then(|value| value["name"].as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    for_videos: detail
                        .and_then(|value| value["forVideos"].as_bool())
                        .unwrap_or(true),
                    hex,
                }
            })
            .collect())
    }

    /// Names a brand colour and says whether videos use it. The colour must be in the palette.
    pub fn set_brand_color_details(
        &self,
        workspace_id: i64,
        hex: &str,
        name: &str,
        for_videos: bool,
    ) -> rusqlite::Result<Vec<BrandColor>> {
        let hex = hex.to_ascii_uppercase();
        let name = name.trim();
        if name.chars().count() > 40 || !self.brand_colors(workspace_id)?.contains(&hex) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "UPDATE workspaces SET brand_color_details_json =
                 json_set(brand_color_details_json, '$.' || json_quote(?2), json_object('name', ?3, 'forVideos', json(?4)))
             WHERE id = ?1 RETURNING id",
            rusqlite::params![workspace_id, hex, name, if for_videos { "true" } else { "false" }],
            |_| Ok(()),
        )?;
        self.brand_palette(workspace_id)
    }

    pub fn set_brand_colors(
        &self,
        workspace_id: i64,
        colors: &[String],
    ) -> rusqlite::Result<Vec<String>> {
        let mut seen = std::collections::HashSet::new();
        let mut normalized = Vec::new();
        for color in colors {
            let hex = color
                .strip_prefix('#')
                .filter(|value| {
                    value.len() == 6 && value.bytes().all(|digit| digit.is_ascii_hexdigit())
                })
                .ok_or(rusqlite::Error::InvalidQuery)?;
            let color = format!("#{}", hex.to_ascii_uppercase());
            if seen.insert(color.clone()) {
                normalized.push(color);
            }
        }
        let json = serde_json::to_string(&normalized)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "UPDATE workspaces SET brand_colors_json = ?2,
                 brand_color_details_json = COALESCE((
                     SELECT json_group_object(detail.key, json(detail.value))
                     FROM json_each(brand_color_details_json) AS detail
                     WHERE detail.key IN (SELECT value FROM json_each(?2))), '{}')
             WHERE id = ?1 RETURNING id",
            rusqlite::params![workspace_id, json],
            |_| Ok(()),
        )?;
        Ok(normalized)
    }

    pub fn create_profile(
        &self,
        workspace_id: i64,
        name: &str,
        guidance: &str,
        color: Option<&str>,
    ) -> rusqlite::Result<Profile> {
        self.0.query_row(
            "INSERT INTO profiles (workspace_id, name, guidance, color)
             VALUES (?1, ?2, ?3, ?4) RETURNING id, workspace_id, name, guidance, color, description",
            rusqlite::params![workspace_id, name.trim(), guidance, color],
            Self::read_profile,
        )
    }

    pub fn update_profile(
        &self,
        workspace_id: i64,
        profile_id: i64,
        name: &str,
        guidance: &str,
        color: Option<&str>,
    ) -> rusqlite::Result<Profile> {
        self.0.query_row(
            "UPDATE profiles SET name = ?3, guidance = ?4, color = ?5
             WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, name, guidance, color, description",
            rusqlite::params![workspace_id, profile_id, name.trim(), guidance, color],
            Self::read_profile,
        )
    }

    pub fn list_profiles(&self, workspace_id: i64) -> rusqlite::Result<Vec<Profile>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, guidance, color, description FROM profiles
                 WHERE workspace_id = ?1 ORDER BY id",
            )?
            .query_map([workspace_id], Self::read_profile)?
            .collect()
    }

    fn read_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
        Ok(Profile {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            guidance: row.get(3)?,
            color: row.get(4)?,
            description: row.get(5)?,
        })
    }

    fn read_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
        Ok(Workspace {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            icon: row.get(3)?,
            image_asset_id: row.get(4)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_lists_workspaces() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        assert!(store.list().unwrap().is_empty());
        let workspace = store.create("  Studio  ").unwrap();
        assert_eq!(workspace.name, "Studio");
        assert_eq!(store.list().unwrap(), vec![workspace]);
    }

    #[test]
    fn workspaces_start_with_a_preset_look_that_can_be_changed() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap();
        assert!(WORKSPACE_ICONS.contains(&workspace.icon.as_deref().unwrap()));
        assert!(WORKSPACE_COLORS.contains(&workspace.color.as_deref().unwrap()));
        let changed = store
            .set_workspace_appearance(workspace.id, "#a1b2c3", "music", None)
            .unwrap();
        assert_eq!(changed.color.as_deref(), Some("#A1B2C3"));
        assert_eq!(changed.icon.as_deref(), Some("music"));
        assert!(
            store
                .set_workspace_appearance(workspace.id, "red", "music", None)
                .is_err()
        );
        assert!(
            store
                .set_workspace_appearance(workspace.id, "#A1B2C3", "skull", None)
                .is_err()
        );
        assert!(
            store
                .set_workspace_appearance(workspace.id, "#A1B2C3", "music", Some(99))
                .is_err()
        );
        assert_eq!(
            store.rename_workspace(workspace.id, " Lab ").unwrap().name,
            "Lab"
        );
        assert!(store.rename_workspace(workspace.id, "  ").is_err());
    }

    #[test]
    fn rejects_blank_names_without_creating_a_record() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        assert!(store.create(" \t ").is_err());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn migrates_an_existing_workspace_database_and_scopes_profiles() {
        let path = std::env::temp_dir().join(format!(
            "arrumo-profile-migration-{}.sqlite",
            std::process::id()
        ));
        let old = Connection::open(&path).unwrap();
        old.execute_batch(include_str!("../migrations/001-create-workspaces.sql"))
            .unwrap();
        old.execute(
            "INSERT INTO workspaces (name) VALUES ('Existing studio')",
            [],
        )
        .unwrap();
        drop(old);

        let store = WorkspaceStore::open(&path).unwrap();
        let existing = &store.list().unwrap()[0];
        assert_eq!(existing.name, "Existing studio");
        assert_eq!(store.list_output_formats(existing.id).unwrap().len(), 2);
        let other = store.create("Other studio").unwrap();
        let profile = store
            .create_profile(existing.id, "  Reviews  ", "## Keep demos", Some("#532A9D"))
            .unwrap();
        assert_eq!(profile.name, "Reviews");
        assert_eq!(profile.guidance, "## Keep demos");
        assert_eq!(profile.color.as_deref(), Some("#532A9D"));
        assert!(store.list_profiles(other.id).unwrap().is_empty());
        assert!(
            store
                .create_profile(other.id + 1, "Orphan", "", None)
                .is_err()
        );
        assert!(
            store
                .create_profile(other.id, "Bad color", "", Some("#nope00"))
                .is_err()
        );
        drop(store);

        let reopened = WorkspaceStore::open(&path).unwrap();
        assert_eq!(reopened.list_profiles(existing.id).unwrap(), vec![profile]);
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn updates_only_the_profile_in_the_requested_workspace() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let first = store.create("First").unwrap();
        let second = store.create("Second").unwrap();
        let profile = store
            .create_profile(first.id, "Reviews", "# Notes", Some("#532A9D"))
            .unwrap();
        assert!(matches!(
            store.update_profile(second.id, profile.id, "Wrong", "", None),
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
        assert!(
            store
                .update_profile(first.id, profile.id, " ", "", None)
                .is_err()
        );
        let profile_id = profile.id;
        assert_eq!(store.list_profiles(first.id).unwrap(), vec![profile]);
        let updated = store
            .update_profile(first.id, profile_id, "  Tutorials ", "## New", None)
            .unwrap();
        assert_eq!(updated.id, profile_id);
        assert_eq!(updated.name, "Tutorials");
        assert_eq!(updated.guidance, "## New");
        assert_eq!(updated.color, None);
        assert_eq!(store.list_profiles(first.id).unwrap(), vec![updated]);
    }

    #[test]
    fn context_preserves_explicit_empty_and_stays_in_its_workspace() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let first = store.create("Studio").unwrap().id;
        let second = store.create("Other").unwrap().id;
        assert_eq!(store.workspace_context(first).unwrap(), None);
        store
            .set_workspace_context(first, "## Audience\nCreators")
            .unwrap();
        assert_eq!(
            store.workspace_context(first).unwrap().as_deref(),
            Some("## Audience\nCreators")
        );
        assert_eq!(store.workspace_context(second).unwrap(), None);
        store.set_workspace_context(first, "").unwrap();
        assert_eq!(store.workspace_context(first).unwrap().as_deref(), Some(""));
        assert!(store.set_workspace_context(i64::MAX, "").is_err());
    }

    #[test]
    fn palette_is_scoped_ordered_and_validated_before_replacing() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let main = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let colors = ["#a1b2c3", "#020304", "#A1B2C3"].map(String::from);
        assert_eq!(
            store.set_brand_colors(main, &colors).unwrap(),
            ["#A1B2C3", "#020304"]
        );
        assert!(store.set_brand_colors(main, &["#ZZ0000".into()]).is_err());
        let palette = store
            .set_brand_color_details(main, &colors[0].to_ascii_lowercase(), " Coral ", false)
            .unwrap();
        assert_eq!(
            (palette[0].name.as_str(), palette[0].for_videos),
            ("Coral", false)
        );
        assert!(
            palette[1..]
                .iter()
                .all(|color| color.for_videos && color.name.is_empty())
        );
        assert!(
            !store
                .video_brand_colors(main)
                .unwrap()
                .contains(&palette[0].hex)
        );
        assert!(
            store
                .set_brand_color_details(main, "#ABCDEF", "Missing", true)
                .is_err()
        );
        let kept = palette[1..]
            .iter()
            .map(|color| color.hex.clone())
            .collect::<Vec<_>>();
        store.set_brand_colors(main, &kept).unwrap();
        store
            .set_brand_colors(main, &[palette[0].hex.clone()])
            .unwrap();
        assert!(
            store.brand_palette(main).unwrap()[0].for_videos,
            "removing a color forgets its details"
        );
        store.set_brand_colors(main, &colors).unwrap();
        assert_eq!(store.brand_colors(main).unwrap(), ["#A1B2C3", "#020304"]);
        assert!(store.brand_colors(other).unwrap().is_empty());
        assert!(store.set_brand_colors(main, &[]).unwrap().is_empty());
        assert!(store.brand_colors(main).unwrap().is_empty());
    }
}
