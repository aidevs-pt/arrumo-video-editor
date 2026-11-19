use std::{collections::BTreeMap, error::Error, io};

use rusqlite::params;
use serde::Serialize;
use serde_json::Value;

use crate::{PluginSettingKind, PluginSettingScope, WorkspaceStore};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPluginSetting {
    pub app: Option<Value>,
    pub workspace_override: Option<Value>,
    pub effective: Option<Value>,
    pub source: Option<PluginSettingScope>,
}

impl WorkspaceStore {
    pub fn plugin_settings(
        &self,
        plugin_id: &str,
        workspace_id: Option<i64>,
    ) -> Result<BTreeMap<String, ResolvedPluginSetting>, Box<dyn Error>> {
        let transaction = self.0.unchecked_transaction()?;
        self.check_plugin_settings_workspace(workspace_id)?;
        let manifest = self.plugin_manifest(plugin_id)?;
        let mut statement = transaction.prepare(
            "SELECT setting_id, workspace_id, value FROM plugin_setting_values
             WHERE plugin_id = ?1 AND (workspace_id IS NULL OR workspace_id = ?2)",
        )?;
        let rows = statement.query_map(params![plugin_id, workspace_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut settings: BTreeMap<_, _> = manifest
            .settings
            .unwrap_or_default()
            .into_iter()
            .filter(|setting| setting.kind != PluginSettingKind::Secret)
            .map(|setting| {
                (
                    setting.id,
                    (
                        setting.kind,
                        setting.scope,
                        ResolvedPluginSetting {
                            app: None,
                            workspace_override: None,
                            effective: None,
                            source: None,
                        },
                    ),
                )
            })
            .collect();
        for row in rows {
            let (id, scope, value) = row?;
            if let Some((kind, declared_scope, state)) = settings.get_mut(&id)
                && (scope.is_none() || *declared_scope == PluginSettingScope::Workspace)
            {
                let value = serde_json::from_str(&value)?;
                if accepts_value(*kind, &value) {
                    if scope.is_some() {
                        state.workspace_override = Some(value);
                    } else {
                        state.app = Some(value);
                    }
                }
            }
        }
        Ok(settings
            .into_iter()
            .map(|(id, (_, _, mut state))| {
                if let Some(value) = &state.workspace_override {
                    state.effective = Some(value.clone());
                    state.source = Some(PluginSettingScope::Workspace);
                } else if let Some(value) = &state.app {
                    state.effective = Some(value.clone());
                    state.source = Some(PluginSettingScope::App);
                }
                (id, state)
            })
            .collect())
    }

    pub fn set_plugin_setting(
        &self,
        plugin_id: &str,
        setting_id: &str,
        workspace_id: Option<i64>,
        value: Option<&Value>,
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.0.unchecked_transaction()?;
        self.check_plugin_settings_workspace(workspace_id)?;
        let manifest = self.plugin_manifest(plugin_id)?;
        let setting = manifest
            .settings
            .unwrap_or_default()
            .into_iter()
            .find(|setting| setting.id == setting_id)
            .ok_or_else(|| invalid_setting("Unknown plugin setting"))?;
        if setting.kind == PluginSettingKind::Secret {
            return Err(invalid_setting("Secret settings require credential storage").into());
        }
        if workspace_id.is_some() && setting.scope == PluginSettingScope::App {
            return Err(
                invalid_setting("Plugin setting can only be configured at app scope").into(),
            );
        }
        if let Some(value) = value {
            if !accepts_value(setting.kind, value) {
                return Err(
                    invalid_setting("Plugin setting value has the wrong type or size").into(),
                );
            }
            transaction.execute(
                "INSERT INTO plugin_setting_values (plugin_id, setting_id, workspace_id, value)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET value = excluded.value",
                params![plugin_id, setting_id, workspace_id, value.to_string()],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM plugin_setting_values
                 WHERE plugin_id = ?1 AND setting_id = ?2 AND workspace_id IS ?3",
                params![plugin_id, setting_id, workspace_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn check_plugin_settings_workspace(&self, workspace_id: Option<i64>) -> rusqlite::Result<()> {
        if let Some(id) = workspace_id {
            self.0
                .query_row("SELECT id FROM workspaces WHERE id = ?1", [id], |_| Ok(()))?;
        }
        Ok(())
    }
}

fn accepts_value(kind: PluginSettingKind, value: &Value) -> bool {
    match kind {
        PluginSettingKind::String => value.as_str().is_some_and(|value| value.len() <= 16 * 1024),
        PluginSettingKind::Boolean => value.is_boolean(),
        PluginSettingKind::Integer => value.as_i64().is_some(),
        PluginSettingKind::Secret => false,
    }
}

fn invalid_setting(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
