use std::io;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::{PluginManifest, WorkspaceStore};

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
        if let Some(expected) = expected_sha256 {
            tx.query_row(
                "SELECT 1 FROM installed_plugin_manifests WHERE plugin_id = ?1 AND package_sha256 = ?2",
                params![plugin_id, expected],
                |_| Ok(()),
            ).map_err(|_| io::Error::other("Plugin package changed after review"))?;
        }
        if allowed {
            let (manifest, digest): (Vec<u8>, String) = tx
                .query_row(
                    "SELECT manifest, package_sha256 FROM installed_plugin_manifests
                 WHERE plugin_id = ?1 AND package_sha256 IS NOT NULL",
                    [plugin_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|_| io::Error::other("Plugin package unavailable"))?;
            let manifest = PluginManifest::parse(&manifest)
                .map_err(|_| io::Error::other("Plugin manifest invalid"))?;
            if manifest.id != plugin_id || !manifest.requests_workspace_context() {
                return Err(io::Error::other(
                    "Plugin does not request workspace context",
                ));
            }
            tx.execute(
                "INSERT INTO plugin_grants (plugin_id, package_sha256, workspace_id, permission)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET package_sha256 = excluded.package_sha256",
                params![plugin_id, digest, workspace_id, CONTEXT_PERMISSION],
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
        self.0.query_row(
            "SELECT 1 FROM plugin_grants AS grant
             JOIN installed_plugin_manifests AS plugin
               ON plugin.plugin_id = grant.plugin_id AND plugin.package_sha256 = grant.package_sha256
             WHERE grant.plugin_id = ?1 AND grant.workspace_id = ?2 AND grant.permission = ?3",
            params![plugin_id, workspace_id, CONTEXT_PERMISSION],
            |_| Ok(()),
        ).optional().map(|found| found.is_some()).map_err(|_| grant_error())
    }
}

fn grant_error() -> io::Error {
    io::Error::other("Could not access plugin grants")
}
