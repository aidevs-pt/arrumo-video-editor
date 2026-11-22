use std::{error::Error, io};

use serde::Serialize;
use serde_json::Value;

use crate::{PluginAction, WorkspaceStore};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginAction {
    pub plugin_id: String,
    pub plugin_name: String,
    pub package_sha256: String,
    pub action: PluginAction,
}

impl WorkspaceStore {
    pub fn plugin_actions(&self) -> Result<Vec<InstalledPluginAction>, Box<dyn Error>> {
        let mut actions = Vec::new();
        for plugin in self.active_plugin_catalog()? {
            let (Some(package), Some(declared)) = (plugin.package, plugin.manifest.actions) else {
                continue;
            };
            actions.extend(declared.into_iter().map(|action| InstalledPluginAction {
                plugin_id: plugin.manifest.id.clone(),
                plugin_name: plugin.manifest.name.clone(),
                package_sha256: package.sha256.clone(),
                action,
            }));
        }
        Ok(actions)
    }

    pub fn invoke_plugin_action(
        &self,
        plugin_id: &str,
        action_id: &str,
        workspace_id: Option<i64>,
        input: &Value,
    ) -> io::Result<Value> {
        self.invoke_plugin(plugin_id, workspace_id, input, Some((action_id, None)))
    }

    pub fn invoke_reviewed_plugin_action(
        &self,
        plugin_id: &str,
        action_id: &str,
        package_sha256: &str,
        workspace_id: Option<i64>,
        input: &Value,
    ) -> io::Result<Value> {
        self.invoke_plugin(
            plugin_id,
            workspace_id,
            input,
            Some((action_id, Some(package_sha256))),
        )
    }
}
