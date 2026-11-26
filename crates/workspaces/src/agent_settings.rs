use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{AgentPlugin, WorkspaceStore, agent_plugin, agent_plugins};

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderSource {
    App,
    Workspace,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AgentSettings {
    pub app_provider: Option<String>,
    pub workspace_override: Option<String>,
    pub effective_provider: Option<String>,
    pub source: Option<AgentProviderSource>,
}

impl WorkspaceStore {
    pub fn agent_settings(&self, workspace_id: Option<i64>) -> rusqlite::Result<AgentSettings> {
        let app_provider: Option<String> = self
            .0
            .query_row(
                "SELECT plugin_id FROM app_agent_plugins WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let workspace_override: Option<String> = if let Some(id) = workspace_id {
            self.0.query_row(
                "SELECT agent_plugin_id FROM workspaces WHERE id = ?1",
                [id],
                |row| row.get(0),
            )?
        } else {
            None
        };
        Ok(AgentSettings {
            effective_provider: workspace_override.clone().or_else(|| app_provider.clone()),
            source: workspace_override
                .as_ref()
                .map(|_| AgentProviderSource::Workspace)
                .or_else(|| app_provider.as_ref().map(|_| AgentProviderSource::App)),
            app_provider,
            workspace_override,
        })
    }

    /// Points a run at the agent selected now, with that agent's settings, so continuing a video
    /// after switching agents uses the new one. With no agent selected the run keeps its own.
    pub(crate) fn adopt_current_agent(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<()> {
        let settings = self.agent_settings(Some(workspace_id))?;
        let Some(provider) = settings.effective_provider.as_deref() else {
            return Ok(());
        };
        let options = self.agent_options(workspace_id, provider);
        let to_json = |value| {
            serde_json::to_string(value)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
        };
        self.0.execute(
            "UPDATE generation_runs
             SET snapshot_json = json_set(snapshot_json, '$.agent_settings', json(?3), '$.agent_options', json(?4))
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, run_id, to_json(&serde_json::json!(settings))?, to_json(&serde_json::json!(options))?],
        )?;
        Ok(())
    }

    /// The agent plugins that are turned on.
    pub fn active_agent_plugins(&self) -> rusqlite::Result<Vec<&'static AgentPlugin>> {
        let activation = self.plugin_activation()?;
        Ok(agent_plugins()
            .iter()
            .filter(|plugin| activation.get(&plugin.id).copied().unwrap_or(true))
            .collect())
    }

    /// Selects the editing agent; an agent that is turned off can't be chosen.
    pub fn set_agent_provider(
        &self,
        workspace_id: Option<i64>,
        provider: Option<&str>,
    ) -> rusqlite::Result<()> {
        let provider = provider
            .map(|id| {
                let plugin = agent_plugin(id).ok_or(rusqlite::Error::InvalidQuery)?;
                self.active_agent_plugins()?
                    .iter()
                    .any(|active| active.id == plugin.id)
                    .then_some(plugin.id.as_str())
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .transpose()?;
        if let Some(id) = workspace_id {
            self.0.query_row(
                "UPDATE workspaces SET agent_plugin_id = ?2 WHERE id = ?1 RETURNING id",
                params![id, provider],
                |_| Ok(()),
            )
        } else {
            if let Some(provider) = provider {
                self.0.execute(
                    "INSERT INTO app_agent_plugins (id, plugin_id) VALUES (1, ?1)
                     ON CONFLICT(id) DO UPDATE SET plugin_id = excluded.plugin_id",
                    [provider],
                )?;
            } else {
                self.0
                    .execute("DELETE FROM app_agent_plugins WHERE id = 1", [])?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DraftSettings, ProductionStage, ReviewDecision, RunControl, RunOptions, WorkspaceStore,
    };

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn continuing_a_video_uses_the_agent_selected_now() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        store.set_agent_provider(None, Some("claude-code")).unwrap();
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Explainer".into(),
                    profile_id: None,
                    intent: "Explain".into(),
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap();
        let agent = |store: &WorkspaceStore| {
            let run = store.generation_run(workspace, run.id).unwrap().unwrap();
            run.snapshot["agent_settings"]["effective_provider"]
                .as_str()
                .map(str::to_owned)
        };
        assert_eq!(agent(&store).as_deref(), Some("claude-code"));

        // Awaiting feedback: the review decision continues with the new agent.
        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let script = serde_json::json!({"markdown": "## Hook"});
        let result = store
            .submit_production_result(workspace, run.id, attempt, ProductionStage::Script, &script)
            .unwrap();
        store.set_agent_provider(None, Some("codex")).unwrap();
        store
            .review_production_result(workspace, run.id, result, ReviewDecision::Approved, "")
            .unwrap();
        assert_eq!(agent(&store).as_deref(), Some("codex"));
        let snapshot = store
            .generation_run(workspace, run.id)
            .unwrap()
            .unwrap()
            .snapshot;
        assert_eq!(
            snapshot["agent_options"],
            serde_json::json!(store.agent_options(workspace, "codex"))
        );

        // Paused: resuming continues with the new agent, here a workspace override.
        store
            .control_generation(workspace, run.id, RunControl::Pause)
            .unwrap();
        store
            .set_agent_provider(Some(workspace), Some("claude-code"))
            .unwrap();
        assert_eq!(
            agent(&store).as_deref(),
            Some("codex"),
            "pausing keeps the agent"
        );
        store
            .control_generation(workspace, run.id, RunControl::Resume)
            .unwrap();
        assert_eq!(agent(&store).as_deref(), Some("claude-code"));

        // With no agent selected any more, the run keeps the one it has.
        store
            .control_generation(workspace, run.id, RunControl::Pause)
            .unwrap();
        store.set_agent_provider(Some(workspace), None).unwrap();
        store.set_agent_provider(None, None).unwrap();
        store
            .control_generation(workspace, run.id, RunControl::Resume)
            .unwrap();
        assert_eq!(agent(&store).as_deref(), Some("claude-code"));
    }
}
