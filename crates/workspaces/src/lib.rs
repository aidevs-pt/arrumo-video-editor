use std::{path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

mod agent_cli;
mod agent_plugins;
mod agent_settings;
mod assets;
mod brand_assets;
mod clip_render;
mod completed_video;
mod completed_video_feedback;
mod delivery_export;
mod delivery_verification;
mod format_deliveries;
mod generation_notifications;
mod generation_plan;
mod generation_queue;
mod generation_worker;
mod mcp;
mod media_worker;
mod output_formats;
mod plugin_actions;
mod plugin_catalog;
mod plugin_credentials;
mod plugin_grants;
mod plugin_package;
mod plugin_registry;
mod plugin_runtime;
mod plugin_schema;
mod plugin_settings;
mod plugin_surfaces;
mod preview_settings;
mod production_progress;
mod profile_deliveries;
mod profile_formats;
mod provider_preferences;
mod transcription;
mod video_drafts;
mod workspace_feedback;

pub use agent_cli::{AgentLoginStatus, agent_cli_detected, check_agent_login};
pub use agent_plugins::{AgentPlugin, agent_plugin, agent_plugins};
pub use agent_settings::{AgentProviderSource, AgentSettings};
pub use assets::{ImagePreview, WorkspaceAsset, asset_path, asset_root};
pub use brand_assets::{BrandAsset, BrandAssetRole};
pub use completed_video::CompletedVideo;
pub use completed_video_feedback::CompletedVideoFeedback;
pub use delivery_verification::{VerifiedDelivery, VerifiedOutput, delivery_directory};
pub use format_deliveries::{
    FormatDelivery, PortraitDerivatives, PortraitSource, RequiredOutput, SupplementalOutputs,
};
pub use generation_notifications::{GenerationInbox, GenerationNotification};
pub use generation_plan::{GenerationPlan, PlanStep, ProductionStage};
pub use generation_queue::{GenerationRun, PendingReview, ReplayMode, RunOptions};
pub use generation_worker::{GenerationActivity, GenerationClaim, RunControl};
pub use mcp::serve_mcp;
pub use media_worker::MediaWork;
pub use output_formats::{OutputFormat, OutputFormatSettings};
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
    PluginAction, PluginActionScope, PluginCode, PluginSetting, PluginSettingKind,
    PluginSettingScope, PluginSurfacePlacement,
};
pub use plugin_settings::ResolvedPluginSetting;
pub use plugin_surfaces::InstalledPluginSurface;
pub use preview_settings::PreviewSettings;
pub use production_progress::{ProductionProgress, ProductionResult, ReviewDecision};
pub use profile_deliveries::{DeliveryOverrides, ProfileDelivery};
pub use profile_formats::{DimensionOverrides, ProfileFormat};
pub use provider_preferences::{CapabilityProvider, ProviderPreferences};
pub use transcription::{ProviderTranscript, Transcript, TranscriptWord};
pub use video_drafts::{DraftSettings, VideoDraft};
pub use workspace_feedback::WorkspaceFeedback;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub guidance: String,
    pub color: Option<String>,
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
        ];
        let pending = usize::try_from(version)
            .ok()
            .and_then(|version| migrations.get(version..))
            .ok_or(rusqlite::Error::InvalidQuery)?;
        for migration_sql in pending {
            migration.execute_batch(migration_sql)?;
        }
        migration.commit()?;
        Ok(Self(connection))
    }

    pub fn create(&self, name: &str) -> rusqlite::Result<Workspace> {
        let name = name.trim();
        self.0.query_row(
            "INSERT INTO workspaces (name) VALUES (?1) RETURNING id, name",
            [name],
            Self::read_workspace,
        )
    }

    pub fn list(&self) -> rusqlite::Result<Vec<Workspace>> {
        self.0
            .prepare("SELECT id, name FROM workspaces ORDER BY id")?
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
            "UPDATE workspaces SET brand_colors_json = ?2 WHERE id = ?1 RETURNING id",
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
             VALUES (?1, ?2, ?3, ?4) RETURNING id, workspace_id, name, guidance, color",
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
             RETURNING id, workspace_id, name, guidance, color",
            rusqlite::params![workspace_id, profile_id, name.trim(), guidance, color],
            Self::read_profile,
        )
    }

    pub fn list_profiles(&self, workspace_id: i64) -> rusqlite::Result<Vec<Profile>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, guidance, color FROM profiles
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
        })
    }

    fn read_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
        Ok(Workspace {
            id: row.get(0)?,
            name: row.get(1)?,
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
        assert_eq!(store.brand_colors(main).unwrap(), ["#A1B2C3", "#020304"]);
        assert!(store.brand_colors(other).unwrap().is_empty());
        assert!(store.set_brand_colors(main, &[]).unwrap().is_empty());
        assert!(store.brand_colors(main).unwrap().is_empty());
    }
}
