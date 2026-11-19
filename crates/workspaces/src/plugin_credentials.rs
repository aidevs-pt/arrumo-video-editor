use std::io;

use serde::Serialize;
use zeroize::Zeroizing;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore, plugin_catalog::bundled_manifests,
};

pub const MAX_PLUGIN_SECRET_BYTES: usize = 2048;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCredentialStatus {
    pub app_configured: bool,
    pub workspace_configured: bool,
    pub source: Option<PluginSettingScope>,
}

impl WorkspaceStore {
    pub fn plugin_credential_status(
        &self,
        plugin_id: &str,
        setting_id: &str,
        workspace_id: Option<i64>,
    ) -> io::Result<PluginCredentialStatus> {
        let keys = self.plugin_credential_keys(plugin_id, setting_id, workspace_id)?;
        let app_configured = credential_exists(&keys.app)?;
        let workspace_configured = keys
            .workspace
            .as_deref()
            .map(credential_exists)
            .transpose()?
            .unwrap_or(false);
        let source = if workspace_configured {
            Some(PluginSettingScope::Workspace)
        } else if app_configured {
            Some(PluginSettingScope::App)
        } else {
            None
        };
        Ok(PluginCredentialStatus {
            app_configured,
            workspace_configured,
            source,
        })
    }

    pub fn set_plugin_credential(
        &self,
        plugin_id: &str,
        setting_id: &str,
        workspace_id: Option<i64>,
        secret: Option<Zeroizing<Vec<u8>>>,
    ) -> io::Result<()> {
        if secret.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > MAX_PLUGIN_SECRET_BYTES || value.contains(&0)
        }) {
            return Err(invalid_credential("Invalid credential length or contents"));
        }
        let keys = self.plugin_credential_keys(plugin_id, setting_id, workspace_id)?;
        if workspace_id.is_some() && keys.workspace.is_none() {
            return Err(invalid_credential("Credential requires app scope"));
        }
        save_credential(
            keys.workspace.as_deref().unwrap_or(&keys.app),
            secret.as_deref().map(Vec::as_slice),
        )
    }

    fn plugin_credential_keys(
        &self,
        plugin_id: &str,
        setting_id: &str,
        workspace_id: Option<i64>,
    ) -> io::Result<CredentialKeys> {
        let setting = bundled_manifests()
            .iter()
            .find(|plugin| plugin.id == plugin_id)
            .and_then(|plugin| plugin.settings.as_ref())
            .and_then(|settings| settings.iter().find(|setting| setting.id == setting_id))
            .filter(|setting| setting.kind == PluginSettingKind::Secret)
            .ok_or_else(|| invalid_credential("Unknown bundled plugin credential"))?;
        if let Some(id) = workspace_id {
            self.0
                .query_row("SELECT id FROM workspaces WHERE id = ?1", [id], |_| Ok(()))
                .map_err(|_| invalid_credential("Unknown workspace"))?;
        }
        let namespace: String = self
            .0
            .query_row(
                "SELECT namespace FROM credential_namespace WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|_| io::Error::other("Credential namespace unavailable"))?;
        let base = format!("{namespace}/{plugin_id}/{setting_id}");
        let workspace = workspace_id.filter(|_| setting.scope == PluginSettingScope::Workspace);
        Ok(CredentialKeys {
            app: format!("{base}/app"),
            workspace: workspace.map(|id| format!("{base}/workspace/{id}")),
        })
    }
}

struct CredentialKeys {
    app: String,
    workspace: Option<String>,
}

fn invalid_credential(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn credential_entry(key: &str) -> io::Result<keyring::Entry> {
    keyring::Entry::new("pt.aidevs.arrumo.plugin-credentials", key)
        .map_err(|_| io::Error::other("System credential store unavailable"))
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn credential_exists(key: &str) -> io::Result<bool> {
    match credential_entry(key)?.get_secret() {
        Ok(secret) => Ok(!Zeroizing::new(secret).is_empty()),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err(io::Error::other("Could not access system credential store")),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn save_credential(key: &str, secret: Option<&[u8]>) -> io::Result<()> {
    let entry = credential_entry(key)?;
    let result = if let Some(secret) = secret {
        entry.set_secret(secret)
    } else {
        match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            result => result,
        }
    };
    result.map_err(|_| io::Error::other("Could not update system credential store"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn credential_exists(_: &str) -> io::Result<bool> {
    Err(io::Error::other("System credential store unavailable"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn save_credential(_: &str, _: Option<&[u8]>) -> io::Result<()> {
    Err(io::Error::other("System credential store unavailable"))
}
