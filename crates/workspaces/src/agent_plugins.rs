use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Manifest {
    id: String,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    capabilities: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(rename = "agentProvider")]
    provider: ProviderDescriptor,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderDescriptor {
    label: String,
    setup_url: String,
    cli: AgentCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentCli {
    pub executable: String,
    pub auth_status_args: Vec<String>,
}

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
        // These are release inputs copied from the official SDK plugin repositories.
        [
            include_str!("../plugins/codex/manifest.json"),
            include_str!("../plugins/claude-code/manifest.json"),
        ]
        .into_iter()
        .map(|data| {
            let manifest: Manifest = serde_json::from_str(data).expect("bundled plugin manifest");
            let descriptor = manifest.provider;
            assert_eq!(manifest.schema_version, 1);
            assert!(manifest.capabilities.contains_key("agents"));
            assert!(descriptor.setup_url.starts_with("https://"));
            assert!(identifier(&manifest.id) && identifier(&descriptor.cli.executable));
            assert!((1..=4).contains(&descriptor.cli.auth_status_args.len()));
            assert!(
                descriptor
                    .cli
                    .auth_status_args
                    .iter()
                    .all(|arg| identifier(arg))
            );
            AgentPlugin {
                id: manifest.id,
                label: descriptor.label,
                setup_url: descriptor.setup_url,
                cli: descriptor.cli,
            }
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

fn identifier(value: &str) -> bool {
    value.split('-').enumerate().all(|(index, part)| {
        !part.is_empty()
            && (index > 0 || part.starts_with(|c: char| c.is_ascii_lowercase()))
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}
