use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSettingKind {
    String,
    Boolean,
    Integer,
    Secret,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSettingScope {
    App,
    Workspace,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSetting {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: PluginSettingKind,
    pub scope: PluginSettingScope,
    pub required: bool,
}
