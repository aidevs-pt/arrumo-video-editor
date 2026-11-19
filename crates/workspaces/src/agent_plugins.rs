use std::sync::OnceLock;

use serde::Serialize;

use crate::plugin_catalog::{AgentCli, bundled_manifests};

#[derive(Debug, Serialize)]
pub struct AgentPlugin {
    pub id: String,
    pub label: String,
    #[serde(skip)]
    pub setup_url: String,
    #[serde(skip)]
    pub(crate) cli: AgentCli,
}

pub fn agent_plugins() -> &'static [AgentPlugin] {
    static PLUGINS: OnceLock<Vec<AgentPlugin>> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        bundled_manifests()
            .iter()
            .filter_map(|manifest| {
                let descriptor = manifest.agent_provider.as_ref()?;
                Some(AgentPlugin {
                    id: manifest.id.clone(),
                    label: descriptor.label.clone(),
                    setup_url: descriptor.setup_url.clone(),
                    cli: descriptor.cli.clone(),
                })
            })
            .collect()
    })
}

pub fn agent_plugin(id: &str) -> Option<&'static AgentPlugin> {
    // Keep CLI callers and previously captured run snapshots readable across the ID migration.
    let id = if id == "claude_code" {
        "claude-code"
    } else {
        id
    };
    agent_plugins().iter().find(|plugin| plugin.id == id)
}
