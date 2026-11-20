use std::io;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::WorkspaceStore;

const CONTEXT_PERMISSION: &str = "workspace-context-read";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginContextRequest {
    pub plugin_id: String,
    pub plugin_name: String,
    pub package_sha256: String,
    pub granted: bool,
}

impl WorkspaceStore {
    pub fn plugin_context_requests(
        &self,
        workspace_id: i64,
    ) -> io::Result<Vec<PluginContextRequest>> {
        let tx = self.0.unchecked_transaction().map_err(|_| grant_error())?;
        tx.query_row(
            "SELECT id FROM workspaces WHERE id = ?1",
            [workspace_id],
            |_| Ok(()),
        )
        .map_err(|_| io::Error::other("Workspace unavailable"))?;
        let mut requests = Vec::new();
        for plugin in self.plugin_catalog().map_err(|_| grant_error())? {
            if !plugin.manifest.requests_workspace_context() {
                continue;
            }
            if let Some(package) = plugin.package {
                requests.push(PluginContextRequest {
                    granted: self.plugin_context_granted(&plugin.manifest.id, workspace_id)?,
                    plugin_id: plugin.manifest.id,
                    plugin_name: plugin.manifest.name,
                    package_sha256: package.sha256,
                });
            }
        }
        tx.commit().map_err(|_| grant_error())?;
        Ok(requests)
    }

    pub fn apply_reviewed_plugin_context_grant(
        &self,
        plugin_id: &str,
        workspace_id: i64,
        package_sha256: &str,
        allowed: bool,
    ) -> io::Result<()> {
        self.write_plugin_context_grant(plugin_id, workspace_id, Some(package_sha256), allowed)
    }

    pub fn set_plugin_context_grant(
        &self,
        plugin_id: &str,
        workspace_id: i64,
        allowed: bool,
    ) -> io::Result<()> {
        self.write_plugin_context_grant(plugin_id, workspace_id, None, allowed)
    }

    fn write_plugin_context_grant(
        &self,
        plugin_id: &str,
        workspace_id: i64,
        expected_sha256: Option<&str>,
        allowed: bool,
    ) -> io::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)
            .map_err(|_| grant_error())?;
        let package = if allowed || expected_sha256.is_some() {
            Some(self.resolve_plugin_package(plugin_id, expected_sha256)?)
        } else {
            None
        };
        if let Some(package) = package.filter(|_| allowed) {
            if !package.manifest.requests_workspace_context() {
                return Err(io::Error::other(
                    "Plugin does not request workspace context",
                ));
            }
            tx.execute(
                "INSERT INTO plugin_grants (plugin_id, package_sha256, workspace_id, permission)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET package_sha256 = excluded.package_sha256",
                params![plugin_id, package.sha256, workspace_id, CONTEXT_PERMISSION],
            )
            .map_err(|_| grant_error())?;
        } else {
            tx.execute(
                "DELETE FROM plugin_grants WHERE plugin_id = ?1 AND workspace_id = ?2 AND permission = ?3",
                params![plugin_id, workspace_id, CONTEXT_PERMISSION],
            ).map_err(|_| grant_error())?;
        }
        tx.commit().map_err(|_| grant_error())
    }

    pub fn plugin_context_granted(&self, plugin_id: &str, workspace_id: i64) -> io::Result<bool> {
        let grant: Option<String> = self
            .0
            .query_row(
                "SELECT package_sha256 FROM plugin_grants
             WHERE plugin_id = ?1 AND workspace_id = ?2 AND permission = ?3",
                params![plugin_id, workspace_id, CONTEXT_PERMISSION],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| grant_error())?;
        Ok(grant.is_some_and(|digest| {
            self.resolve_plugin_package(plugin_id, Some(&digest))
                .is_ok()
        }))
    }
}

fn grant_error() -> io::Error {
    io::Error::other("Could not access plugin grants")
}
