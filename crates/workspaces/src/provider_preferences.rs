use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    io,
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{WorkspaceStore, plugin_catalog::identifier};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum PreferenceSource {
    BundledDefault,
    App,
    Workspace,
    Run,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProvider {
    pub id: String,
    pub name: String,
    pub bundled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreferences {
    pub available: Vec<CapabilityProvider>,
    pub app: Option<Vec<String>>,
    pub workspace_override: Option<Vec<String>>,
    pub effective: Vec<String>,
    source: PreferenceSource,
}

#[derive(Debug, Serialize)]
pub(crate) struct CapturedProviderOrder {
    pub(crate) effective: Vec<String>,
    source: PreferenceSource,
}

impl WorkspaceStore {
    pub fn provider_preferences(
        &self,
        capability: &str,
        workspace_id: Option<i64>,
    ) -> Result<ProviderPreferences, Box<dyn Error>> {
        let _transaction = self.0.unchecked_transaction()?;
        self.check_preference_scope(capability, workspace_id)?;
        let available = self.capability_providers(capability)?;
        self.resolve_provider_preferences(capability, workspace_id, available)
    }

    // Generation already holds the write transaction that freezes the run inputs.
    pub(crate) fn capture_provider_orders(
        &self,
        workspace_id: i64,
        overrides: &BTreeMap<String, Vec<String>>,
    ) -> Result<BTreeMap<String, CapturedProviderOrder>, Box<dyn Error>> {
        let capabilities = self.capability_index(Some(workspace_id))?;
        for (capability, order) in overrides {
            let available = capabilities
                .get(capability)
                .filter(|_| capability != "agents")
                .ok_or_else(|| invalid_preference("Unsupported per-run capability override"))?;
            validate_provider_order(order, available)?;
        }
        capabilities
            .into_iter()
            .map(|(capability, available)| {
                if let Some(order) = overrides.get(&capability) {
                    return Ok((
                        capability,
                        CapturedProviderOrder {
                            effective: order.clone(),
                            source: PreferenceSource::Run,
                        },
                    ));
                }
                let order =
                    self.resolve_provider_preferences(&capability, Some(workspace_id), available)?;
                Ok((
                    capability,
                    CapturedProviderOrder {
                        effective: order.effective,
                        source: order.source,
                    },
                ))
            })
            .collect()
    }

    pub fn list_provider_preferences(
        &self,
        workspace_id: Option<i64>,
    ) -> Result<BTreeMap<String, ProviderPreferences>, Box<dyn Error>> {
        let _transaction = self.0.unchecked_transaction()?;
        if let Some(id) = workspace_id {
            self.0
                .query_row("SELECT id FROM workspaces WHERE id = ?1", [id], |_| Ok(()))?;
        }
        self.capability_index(workspace_id)?
            .into_iter()
            .map(|(capability, available)| {
                let preference =
                    self.resolve_provider_preferences(&capability, workspace_id, available)?;
                Ok((capability, preference))
            })
            .collect()
    }

    pub(crate) fn capability_index(
        &self,
        workspace_id: Option<i64>,
    ) -> Result<BTreeMap<String, Vec<CapabilityProvider>>, Box<dyn Error>> {
        let mut capabilities: BTreeMap<String, Vec<CapabilityProvider>> = BTreeMap::new();
        for plugin in self.active_plugin_catalog()? {
            for capability in plugin.manifest.capabilities.keys() {
                capabilities
                    .entry(capability.clone())
                    .or_default()
                    .push(CapabilityProvider {
                        id: plugin.manifest.id.clone(),
                        name: plugin.manifest.name.clone(),
                        bundled: plugin.bundled,
                    });
            }
        }
        let mut query = self.0.prepare(
            "SELECT DISTINCT capability FROM plugin_provider_preferences
             WHERE workspace_id IS NULL OR workspace_id = ?1",
        )?;
        for stored in query.query_map([workspace_id], |row| row.get::<_, String>(0))? {
            capabilities.entry(stored?).or_default();
        }
        Ok(capabilities)
    }

    fn resolve_provider_preferences(
        &self,
        capability: &str,
        workspace_id: Option<i64>,
        mut available: Vec<CapabilityProvider>,
    ) -> Result<ProviderPreferences, Box<dyn Error>> {
        available.sort_by(|first, second| first.id.cmp(&second.id));
        let resolve_scope = |scope: Option<i64>| -> Result<Option<Vec<String>>, Box<dyn Error>> {
            let stored: Option<String> = self.0.query_row(
                "SELECT plugin_ids FROM plugin_provider_preferences WHERE capability = ?1 AND workspace_id IS ?2",
                params![capability, scope],
                |row| row.get(0),
            ).optional()?;
            let providers = stored
                .map(|value| serde_json::from_str::<Vec<String>>(&value))
                .transpose()?;
            Ok(providers.map(|mut providers| {
                providers.retain(|id| available.iter().any(|plugin| &plugin.id == id));
                providers
            }))
        };
        let app = resolve_scope(None)?;
        let workspace_override = if workspace_id.is_some() {
            resolve_scope(workspace_id)?
        } else {
            None
        };
        let (effective, source) = if let Some(order) = &workspace_override {
            (order.clone(), PreferenceSource::Workspace)
        } else if let Some(order) = &app {
            (order.clone(), PreferenceSource::App)
        } else {
            let defaults = available
                .iter()
                .filter(|plugin| plugin.bundled)
                .map(|plugin| plugin.id.clone())
                .collect();
            (defaults, PreferenceSource::BundledDefault)
        };
        Ok(ProviderPreferences {
            available,
            app,
            workspace_override,
            effective,
            source,
        })
    }

    pub fn set_provider_preferences(
        &self,
        capability: &str,
        workspace_id: Option<i64>,
        order: Option<&[String]>,
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.0.unchecked_transaction()?;
        self.check_preference_scope(capability, workspace_id)?;
        if let Some(order) = order {
            let available = self.capability_providers(capability)?;
            validate_provider_order(order, &available)?;
            transaction.execute(
                "INSERT INTO plugin_provider_preferences (capability, workspace_id, plugin_ids)
                 VALUES (?1, ?2, ?3) ON CONFLICT DO UPDATE SET plugin_ids = excluded.plugin_ids",
                params![capability, workspace_id, serde_json::to_string(order)?],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM plugin_provider_preferences WHERE capability = ?1 AND workspace_id IS ?2",
                params![capability, workspace_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn capability_providers(
        &self,
        capability: &str,
    ) -> Result<Vec<CapabilityProvider>, Box<dyn Error>> {
        Ok(self
            .active_plugin_catalog()?
            .into_iter()
            .filter(|plugin| plugin.manifest.capabilities.contains_key(capability))
            // An agent plugin does its work through its CLI, so it can't provide anything while
            // the CLI isn't on this computer.
            .filter(|plugin| {
                crate::agent_plugin(&plugin.manifest.id)
                    .is_none_or(|agent| crate::agent_cli::find_agent_cli(agent).is_some())
            })
            .map(|plugin| CapabilityProvider {
                id: plugin.manifest.id,
                name: plugin.manifest.name,
                bundled: plugin.bundled,
            })
            .collect())
    }

    fn check_preference_scope(
        &self,
        capability: &str,
        workspace_id: Option<i64>,
    ) -> io::Result<()> {
        if !identifier(capability) {
            return Err(invalid_preference("Invalid capability ID"));
        }
        if let Some(id) = workspace_id {
            self.0
                .query_row("SELECT id FROM workspaces WHERE id = ?1", [id], |_| Ok(()))
                .map_err(|_| invalid_preference("Workspace not found"))?;
        }
        Ok(())
    }
}

fn validate_provider_order(order: &[String], available: &[CapabilityProvider]) -> io::Result<()> {
    let mut seen = HashSet::new();
    if order
        .iter()
        .any(|id| !seen.insert(id) || !available.iter().any(|provider| &provider.id == id))
    {
        return Err(invalid_preference(
            "Provider order contains a duplicate or an unavailable plugin",
        ));
    }
    Ok(())
}

fn invalid_preference(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

impl WorkspaceStore {
    /// The active plugins whose required settings all have a value for this workspace, so they
    /// can do their work. A plugin missing a key or other required setting is left out.
    pub fn ready_plugins(&self, workspace_id: Option<i64>) -> Result<Vec<String>, Box<dyn Error>> {
        let mut ready = Vec::new();
        for plugin in self.active_plugin_catalog()? {
            let id = &plugin.manifest.id;
            let required: Vec<_> = plugin
                .manifest
                .settings
                .iter()
                .flatten()
                .filter(|setting| setting.required)
                .collect();
            let values = if required
                .iter()
                .any(|setting| setting.kind != crate::PluginSettingKind::Secret)
            {
                self.plugin_settings(id, workspace_id)?
            } else {
                BTreeMap::new()
            };
            let mut all = true;
            for setting in required {
                let filled = if setting.kind == crate::PluginSettingKind::Secret {
                    self.plugin_credential_status(id, &setting.id, workspace_id)?
                        .source
                        .is_some()
                } else {
                    values
                        .get(&setting.id)
                        .and_then(|value| value.effective.as_ref())
                        .is_some_and(|value| {
                            !value.is_null()
                                && value.as_str().is_none_or(|text| !text.trim().is_empty())
                        })
                };
                if !filled {
                    all = false;
                    break;
                }
            }
            if all {
                ready.push(id.clone());
            }
        }
        Ok(ready)
    }
}
