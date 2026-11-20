use std::io;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{MAX_PLUGIN_IO_BYTES, PluginActionScope, WorkspaceStore};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginActionJobRequest {
    pub id: String,
    pub workspace_id: Option<i64>,
    pub plugin_id: String,
    pub action_id: String,
    pub package_sha256: String,
    pub input: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginActionJob {
    id: String,
    workspace_id: Option<i64>,
    plugin_id: String,
    action_id: String,
    package_sha256: String,
    state: String,
    attempt: i64,
    result: Option<Value>,
}

impl WorkspaceStore {
    pub fn queue_plugin_action_job(
        &self,
        request: &PluginActionJobRequest,
    ) -> io::Result<PluginActionJob> {
        let input = serde_json::to_string(&request.input).map_err(|_| job_error())?;
        let envelope = serde_json::to_vec(&serde_json::json!({
            "action": request.action_id, "input": request.input,
        }))
        .map_err(|_| job_error())?;
        if envelope.len() > MAX_PLUGIN_IO_BYTES {
            return Err(io::Error::other(
                "Plugin job input exceeds the action limit",
            ));
        }
        let job_id: Uuid = request
            .id
            .parse()
            .map_err(|_| io::Error::other("Invalid plugin job ID"))?;
        let id = job_id.to_string();
        let transaction = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)
            .map_err(|_| job_error())?;
        let existing: Option<bool> = transaction
            .query_row(
                "SELECT workspace_id IS ?2 AND plugin_id = ?3 AND action_id = ?4
                    AND package_sha256 = ?5 AND input = ?6 FROM plugin_action_jobs WHERE id = ?1",
                params![
                    id,
                    request.workspace_id,
                    request.plugin_id,
                    request.action_id,
                    request.package_sha256,
                    input
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| job_error())?;
        if let Some(same) = existing {
            if !same {
                return Err(io::Error::other(
                    "Plugin job ID already belongs to a different request",
                ));
            }
            return self.plugin_action_job(request.workspace_id, job_id);
        }
        let package =
            self.resolve_plugin_package(&request.plugin_id, Some(&request.package_sha256))?;
        let action = package
            .manifest
            .actions
            .as_ref()
            .and_then(|actions| actions.iter().find(|action| action.id == request.action_id))
            .ok_or_else(|| io::Error::other("Plugin action job requires a declared action"))?;
        if (action.scope == PluginActionScope::Workspace) != request.workspace_id.is_some() {
            return Err(io::Error::other("Plugin action requires a different scope"));
        }
        transaction.execute(
            "INSERT INTO plugin_action_jobs(id, workspace_id, plugin_id, action_id, package_sha256, input)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, request.workspace_id, request.plugin_id, request.action_id, request.package_sha256, input],
        ).map_err(|_| job_error())?;
        transaction.commit().map_err(|_| job_error())?;
        self.plugin_action_job(request.workspace_id, job_id)
    }

    pub fn plugin_action_job(
        &self,
        workspace_id: Option<i64>,
        id: Uuid,
    ) -> io::Result<PluginActionJob> {
        self.0.query_row(
            "SELECT id, workspace_id, plugin_id, action_id, package_sha256, state, attempt, result
             FROM plugin_action_jobs WHERE id = ?1 AND workspace_id IS ?2",
            params![id.to_string(), workspace_id],
            read_job,
        ).map_err(|_| job_error())
    }

    pub fn list_plugin_action_jobs(
        &self,
        workspace_id: Option<i64>,
    ) -> io::Result<Vec<PluginActionJob>> {
        let mut query = self.0.prepare(
            "SELECT id, workspace_id, plugin_id, action_id, package_sha256, state, attempt, result
             FROM plugin_action_jobs WHERE workspace_id IS ?1 ORDER BY created_at DESC, rowid DESC LIMIT 10",
        ).map_err(|_| job_error())?;
        query
            .query_map([workspace_id], read_job)
            .map_err(|_| job_error())?
            .collect::<rusqlite::Result<_>>()
            .map_err(|_| job_error())
    }

    pub fn process_next_plugin_action_job(&self) -> io::Result<Option<PluginActionJob>> {
        let next: Option<(String, Option<i64>)> = self
            .0
            .query_row(
                "SELECT id, workspace_id FROM plugin_action_jobs
             WHERE state = 'queued' OR (state = 'running' AND lease_until <= unixepoch())
             ORDER BY created_at, rowid LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| job_error())?;
        next.map(|(id, workspace)| {
            let id = id.parse().map_err(|_| job_error())?;
            self.process_plugin_action_job(workspace, id)
        })
        .transpose()
    }

    pub fn process_plugin_action_job(
        &self,
        workspace_id: Option<i64>,
        id: Uuid,
    ) -> io::Result<PluginActionJob> {
        let claim: Option<(String, String, String, String, i64)> = self
            .0
            .query_row(
                "UPDATE plugin_action_jobs
             SET state = 'running', attempt = attempt + 1, lease_until = unixepoch() + 120
             WHERE id = ?1 AND workspace_id IS ?2 AND
                (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
             RETURNING plugin_id, action_id, package_sha256, input, attempt",
                params![id.to_string(), workspace_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| job_error())?;
        if let Some((plugin, action, digest, input, attempt)) = claim {
            let input = serde_json::from_str::<Value>(&input).map_err(|_| job_error())?;
            let (state, result) = match self.invoke_reviewed_plugin_action(&plugin, &action, &digest, workspace_id, &input) {
                Ok(result) => ("completed", serde_json::to_string(&result).map_err(|_| job_error())?),
                Err(_) => ("failed", serde_json::json!({"error": "Plugin action could not run for the reviewed package"}).to_string()),
            };
            self.0
                .execute(
                    "UPDATE plugin_action_jobs SET state = ?4, result = ?5, lease_until = NULL
                 WHERE id = ?1 AND workspace_id IS ?2 AND attempt = ?3 AND state = 'running'",
                    params![id.to_string(), workspace_id, attempt, state, result],
                )
                .map_err(|_| job_error())?;
        }
        self.plugin_action_job(workspace_id, id)
    }
}

fn read_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<PluginActionJob> {
    let result: Option<String> = row.get(7)?;
    Ok(PluginActionJob {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        plugin_id: row.get(2)?,
        action_id: row.get(3)?,
        package_sha256: row.get(4)?,
        state: row.get(5)?,
        attempt: row.get(6)?,
        result: result
            .map(|value| serde_json::from_str(&value).map_err(|_| rusqlite::Error::InvalidQuery))
            .transpose()?,
    })
}

fn job_error() -> io::Error {
    io::Error::other("Could not access plugin action job")
}
