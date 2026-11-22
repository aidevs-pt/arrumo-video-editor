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
