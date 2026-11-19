use std::io;

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{PluginManifest, WorkspaceStore};

const CONTEXT_PERMISSION: &str = "workspace-context-read";

impl WorkspaceStore {
    pub fn set_plugin_context_grant(
        &self,
        plugin_id: &str,
        workspace_id: i64,
        allowed: bool,
    ) -> io::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)
            .map_err(|_| grant_error())?;
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
