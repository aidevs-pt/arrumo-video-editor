use rusqlite::{
    OptionalExtension, params,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    Codex,
    ClaudeCode,
}

impl AgentProvider {
    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }
}

impl FromSql for AgentProvider {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "codex" => Ok(Self::Codex),
            "claude_code" => Ok(Self::ClaudeCode),
            _ => Err(FromSqlError::Other("Unknown agent provider".into())),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderSource {
    App,
    Workspace,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AgentSettings {
    pub app_provider: Option<AgentProvider>,
    pub workspace_override: Option<AgentProvider>,
    pub effective_provider: Option<AgentProvider>,
    pub source: Option<AgentProviderSource>,
}

impl WorkspaceStore {
    pub fn agent_settings(&self, workspace_id: Option<i64>) -> rusqlite::Result<AgentSettings> {
        let app_provider: Option<AgentProvider> = self
            .0
            .query_row(
                "SELECT provider FROM app_agent_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let workspace_override: Option<AgentProvider> = if let Some(id) = workspace_id {
            self.0.query_row(
                "SELECT agent_provider FROM workspaces WHERE id = ?1",
                [id],
                |row| row.get(0),
            )?
        } else {
            None
        };
        Ok(AgentSettings {
            app_provider,
            workspace_override,
            effective_provider: workspace_override.or(app_provider),
            source: workspace_override
                .map(|_| AgentProviderSource::Workspace)
                .or_else(|| app_provider.map(|_| AgentProviderSource::App)),
        })
    }

    pub fn set_agent_provider(
        &self,
        workspace_id: Option<i64>,
        provider: Option<AgentProvider>,
    ) -> rusqlite::Result<()> {
        let provider = provider.map(AgentProvider::as_str);
        if let Some(id) = workspace_id {
            self.0.query_row(
                "UPDATE workspaces SET agent_provider = ?2 WHERE id = ?1 RETURNING id",
                params![id, provider],
                |_| Ok(()),
            )
        } else {
            if let Some(provider) = provider {
                self.0.execute(
                    "INSERT INTO app_agent_settings (id, provider) VALUES (1, ?1)
                     ON CONFLICT(id) DO UPDATE SET provider = excluded.provider",
                    [provider],
                )?;
            } else {
                self.0
                    .execute("DELETE FROM app_agent_settings WHERE id = 1", [])?;
            }
            Ok(())
        }
    }
}
