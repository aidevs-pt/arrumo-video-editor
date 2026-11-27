//! Turning installed plugins on and off. An inactive plugin stays installed with its settings,
//! but Arrumo does not run it: it is left out of provider orders (including orders a run captured
//! earlier), action listings and jobs, hooks, frontend surfaces and usage reads.

use std::{collections::BTreeMap, error::Error, io};

use rusqlite::{OptionalExtension, params};

use crate::{WorkspaceStore, agent_plugin, plugin_catalog::CatalogPlugin};

const NOT_INSTALLED: &str = "No installed plugin has this ID";
const APP_AGENT: &str = "This agent is the app's editing agent. Choose another agent first.";
const WORKSPACE_AGENT: &str = "A workspace uses this agent. Choose another agent for it first.";
const RUN_AGENT: &str = "An unfinished video uses this agent. Finish or cancel it first.";

const REFUSALS: [(&str, &str); 4] = [
    ("plugin_missing", NOT_INSTALLED),
    ("plugin_agent_app", APP_AGENT),
    ("plugin_agent_workspace", WORKSPACE_AGENT),
    ("plugin_agent_run", RUN_AGENT),
];

/// Why `set_plugin_active` refused, as one of its fixed messages.
pub(crate) fn activation_refusal(error: &dyn Error) -> Option<&'static str> {
    let message = error.to_string();
    REFUSALS
        .iter()
        .find(|(_, reason)| *reason == message)
        .map(|(_, reason)| *reason)
}

/// Why `set_plugin_active` refused, as a stable code for translated messages.
pub fn plugin_activation_refusal_code(error: &dyn Error) -> Option<&'static str> {
    let message = error.to_string();
    REFUSALS
        .iter()
        .find(|(_, reason)| *reason == message)
        .map(|(code, _)| *code)
}

impl WorkspaceStore {
    /// Stored on/off choices; plugins without one use their default.
    pub(crate) fn plugin_activation(&self) -> rusqlite::Result<BTreeMap<String, bool>> {
        self.0
            .prepare("SELECT plugin_id, active FROM plugin_activation")?
            .query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? == 1)))?
            .collect()
    }

    /// Fills in `active` on catalog entries: bundled plugins default to on, imported ones to off.
    pub(crate) fn mark_active(&self, plugins: &mut [CatalogPlugin]) -> rusqlite::Result<()> {
        let stored = self.plugin_activation()?;
        for plugin in plugins {
            plugin.active = stored
                .get(&plugin.manifest.id)
                .copied()
                .unwrap_or(plugin.bundled);
        }
        Ok(())
    }

    /// The installed plugins Arrumo may run.
    pub fn active_plugin_catalog(&self) -> Result<Vec<CatalogPlugin>, Box<dyn Error>> {
        Ok(self
            .plugin_catalog()?
            .into_iter()
            .filter(|plugin| plugin.active)
            .collect())
    }

    pub(crate) fn plugin_is_active(&self, plugin_id: &str) -> Result<bool, Box<dyn Error>> {
        Ok(self
            .plugin_catalog()?
            .iter()
            .any(|plugin| plugin.active && plugin.manifest.id == plugin_id))
    }

    /// Refuses a provider a caller named when it is installed but turned off.
    pub(crate) fn check_named_provider(&self, plugin_id: &str) -> Result<(), Box<dyn Error>> {
        let off = self
            .plugin_catalog()?
            .into_iter()
            .find(|plugin| plugin.manifest.id == plugin_id && !plugin.active);
        match off {
            Some(plugin) => Err(io::Error::other(format!(
                "{} is turned off. Turn it on in Settings › Plugins.",
                plugin.manifest.name
            ))
            .into()),
            None => Ok(()),
        }
    }

    /// A captured provider order without the installed plugins turned off since.
    pub(crate) fn without_inactive(
        &self,
        order: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let inactive: Vec<String> = self
            .plugin_catalog()?
            .into_iter()
            .filter(|plugin| !plugin.active)
            .map(|plugin| plugin.manifest.id)
            .collect();
        Ok(order
            .into_iter()
            .filter(|id| !inactive.contains(id))
            .collect())
    }

    /// Turns an installed plugin on or off. An agent can't be turned off while the app, a
    /// workspace or an unfinished run uses it.
    pub fn set_plugin_active(&self, plugin_id: &str, active: bool) -> Result<(), Box<dyn Error>> {
        let transaction = self.0.unchecked_transaction()?;
        if !self
            .plugin_catalog()?
            .iter()
            .any(|plugin| plugin.manifest.id == plugin_id)
        {
            return Err(io::Error::new(io::ErrorKind::NotFound, NOT_INSTALLED).into());
        }
        if !active
            && agent_plugin(plugin_id).is_some()
            && let Some(reason) = self.agent_in_use(plugin_id)?
        {
            return Err(io::Error::other(reason).into());
        }
        transaction.execute(
            "INSERT INTO plugin_activation (plugin_id, active) VALUES (?1, ?2)
             ON CONFLICT(plugin_id) DO UPDATE SET active = excluded.active",
            params![plugin_id, i64::from(active)],
        )?;
        transaction.commit()?;
        let _ = self.canvas_status();
        Ok(())
    }

    fn agent_in_use(&self, plugin_id: &str) -> rusqlite::Result<Option<&'static str>> {
        let app: bool = self.0.query_row(
            "SELECT EXISTS (SELECT 1 FROM app_agent_plugins WHERE plugin_id = ?1)",
            [plugin_id],
            |row| row.get(0),
        )?;
        if app {
            return Ok(Some(APP_AGENT));
        }
        let workspace: Option<String> = self
            .0
            .query_row(
                "SELECT name FROM workspaces WHERE agent_plugin_id = ?1 LIMIT 1",
                [plugin_id],
                |row| row.get(0),
            )
            .optional()?;
        if workspace.is_some() {
            return Ok(Some(WORKSPACE_AGENT));
        }
        let running: bool = self.0.query_row(
            "SELECT EXISTS (SELECT 1 FROM generation_runs
             WHERE state IN ('queued', 'running', 'paused')
               AND json_extract(snapshot_json, '$.agent_settings.effective_provider') = ?1)",
            [plugin_id],
            |row| row.get(0),
        )?;
        Ok(running.then_some(RUN_AGENT))
    }

    /// Forgets the on/off choice of an imported plugin when it is uninstalled, so a reinstall
    /// starts off again.
    pub(crate) fn forget_plugin_activation(&self, plugin_id: &str) -> rusqlite::Result<()> {
        if crate::plugin_catalog::bundled_manifests()
            .iter()
            .all(|manifest| manifest.id != plugin_id)
        {
            self.0.execute(
                "DELETE FROM plugin_activation WHERE plugin_id = ?1",
                [plugin_id],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
impl WorkspaceStore {
    /// Installs a plugin ZIP and turns it on, as a creator would before using it.
    pub(crate) fn install_plugin_zip_on(&self, archive: &[u8]) -> CatalogPlugin {
        let mut plugin = self.install_plugin_zip(archive).unwrap();
        self.set_plugin_active(&plugin.manifest.id, true).unwrap();
        plugin.active = true;
        plugin
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::WorkspaceStore;

    const FIXTURE: &[u8] = br#"{
        "schemaVersion": 1,
        "id": "fixture-provider",
        "name": "Fixture Provider",
        "description": "Test fixture",
        "capabilities": {"transcription": {"name": "Transcription", "description": "Test fixture"}}
    }"#;

    fn active(store: &WorkspaceStore, id: &str) -> bool {
        store
            .plugin_catalog()
            .unwrap()
            .into_iter()
            .find(|plugin| plugin.manifest.id == id)
            .unwrap()
            .active
    }

    #[test]
    fn bundled_plugins_start_on_and_imports_start_off() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        assert!(
            store
                .plugin_catalog()
                .unwrap()
                .iter()
                .all(|plugin| plugin.active)
        );
        assert!(!store.install_plugin_manifest(FIXTURE).unwrap().active);
        assert!(!active(&store, "fixture-provider"));
        store.set_plugin_active("fixture-provider", true).unwrap();
        assert!(active(&store, "fixture-provider"));
        // Uninstalling forgets the choice, so a reinstall starts off again.
        assert!(
            store
                .remove_installed_plugin_manifest("fixture-provider")
                .unwrap()
        );
        store.install_plugin_manifest(FIXTURE).unwrap();
        assert!(!active(&store, "fixture-provider"));
        assert!(store.set_plugin_active("missing", true).is_err());
    }

    #[test]
    fn imports_installed_before_the_switch_stay_on() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.sqlite");
        {
            let store = WorkspaceStore::open(&path).unwrap();
            store.install_plugin_manifest(FIXTURE).unwrap();
            store
                .0
                .execute_batch(
                    "DROP TABLE plugin_activation; DROP TABLE agent_usage_observations; DROP TABLE profile_formats_off; DROP TABLE storage_settings; DROP TABLE motion_library_versions;
                     ALTER TABLE motion_library DROP COLUMN version; ALTER TABLE motion_library DROP COLUMN change_note; ALTER TABLE motion_library DROP COLUMN restored_from; ALTER TABLE motion_library DROP COLUMN component_json;
                     DROP TABLE brand_fonts; DROP TABLE agent_executables;
                     ALTER TABLE privacy_settings DROP COLUMN install_reported;
                     ALTER TABLE privacy_settings DROP COLUMN crash_report_id;
                     DROP TRIGGER generation_activity_transition;
                     ALTER TABLE generation_activity DROP COLUMN stage;
                     ALTER TABLE agent_logs DROP COLUMN round; ALTER TABLE agent_sessions DROP COLUMN round;
                     PRAGMA user_version = 63;",
                )
                .unwrap();
        }
        let store = WorkspaceStore::open(&path).unwrap();
        assert!(active(&store, "fixture-provider"));
    }

    #[test]
    fn turned_off_providers_leave_orders_and_captured_runs() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        store.install_plugin_manifest(FIXTURE).unwrap();
        let available = |store: &WorkspaceStore| {
            store
                .provider_preferences("transcription", Some(workspace))
                .unwrap()
                .available
                .into_iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>()
        };
        assert!(!available(&store).contains(&"fixture-provider".to_owned()));
        assert!(
            store
                .set_provider_preferences("transcription", None, Some(&["fixture-provider".into()]))
                .is_err()
        );
        store.set_plugin_active("fixture-provider", true).unwrap();
        assert!(available(&store).contains(&"fixture-provider".to_owned()));
        let captured = vec!["fixture-provider".to_owned(), "elevenlabs".to_owned()];
        assert_eq!(store.without_inactive(captured.clone()).unwrap(), captured);
        store.set_plugin_active("fixture-provider", false).unwrap();
        assert_eq!(store.without_inactive(captured).unwrap(), ["elevenlabs"]);
        // Unknown IDs, such as test fixtures, are not this rule's business.
        assert_eq!(
            store.without_inactive(vec!["other".into()]).unwrap(),
            ["other"]
        );
    }

    #[test]
    fn named_providers_and_captured_renderers_respect_the_switch() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        store.install_plugin_manifest(FIXTURE).unwrap();
        let refused = store.check_named_provider("fixture-provider").unwrap_err();
        assert!(refused.to_string().contains("turned off"));
        store.check_named_provider("elevenlabs").unwrap();
        let snapshot = json!({"motion_renderer": {"plugin_id": "fixture-provider"}});
        assert!(
            !store.renders_motion(&snapshot).unwrap(),
            "falls back to agent animations"
        );
        store.set_plugin_active("fixture-provider", true).unwrap();
        assert!(store.renders_motion(&snapshot).unwrap());
        assert!(!store.renders_motion(&json!({})).unwrap());
    }

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn a_retried_run_whose_agent_was_turned_off_fails_with_the_reason() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        store.set_agent_provider(None, Some("codex")).unwrap();
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
                    title: "Explainer".into(),
                    profile_id: None,
                    intent: "Explain the cube".into(),
                    source_ids: vec![],
                },
            )
            .unwrap()
            .id;
        let run = store
            .enqueue_video_draft(workspace, draft, "k", &crate::RunOptions::default())
            .unwrap()
            .id;
        store
            .0
            .execute("UPDATE generation_runs SET state = 'failed'", [])
            .unwrap();
        store.set_agent_provider(None, None).unwrap();
        store.set_plugin_active("codex", false).unwrap();
        store
            .0
            .execute("UPDATE generation_runs SET state = 'queued'", [])
            .unwrap();
        let error = store
            .process_script_stage(std::path::Path::new(":memory:"), workspace, run)
            .unwrap_err();
        assert!(error.to_string().contains("is turned off"));
        let state: String = store
            .0
            .query_row(
                "SELECT state FROM generation_runs WHERE id = ?1",
                [run],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "failed");
    }

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn an_agent_in_use_cannot_be_turned_off() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        store.set_agent_provider(None, Some("codex")).unwrap();
        let refusal = |store: &WorkspaceStore| {
            store
                .set_plugin_active("codex", false)
                .err()
                .and_then(|error| super::activation_refusal(error.as_ref()))
        };
        assert_eq!(refusal(&store), Some(super::APP_AGENT));
        store.set_agent_provider(None, None).unwrap();
        store
            .set_agent_provider(Some(workspace), Some("codex"))
            .unwrap();
        assert_eq!(refusal(&store), Some(super::WORKSPACE_AGENT));
        store.set_agent_provider(Some(workspace), None).unwrap();
        let mut store = store;
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
                    title: "Explainer".into(),
                    profile_id: None,
                    intent: "Explain the cube".into(),
                    source_ids: vec![],
                },
            )
            .unwrap()
            .id;
        store
            .0
            .execute(
                "INSERT INTO generation_runs (workspace_id, draft_id, request_key, state, snapshot_json)
                 VALUES (?1, ?2, 'k', 'paused', ?3)",
                rusqlite::params![
                    workspace,
                    draft,
                    json!({"agent_settings": {"effective_provider": "codex"}}).to_string()
                ],
            )
            .unwrap();
        assert_eq!(refusal(&store), Some(super::RUN_AGENT));
        store
            .0
            .execute("UPDATE generation_runs SET state = 'completed'", [])
            .unwrap();
        store.set_plugin_active("codex", false).unwrap();
        assert!(
            store
                .active_agent_plugins()
                .unwrap()
                .iter()
                .all(|plugin| plugin.id != "codex")
        );
        assert!(store.set_agent_provider(None, Some("codex")).is_err());
        store.set_plugin_active("codex", true).unwrap();
        store.set_agent_provider(None, Some("codex")).unwrap();
    }
}
