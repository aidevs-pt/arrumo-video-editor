use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSettingKind {
    String,
    Boolean,
    Integer,
    Secret,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCode {
    #[serde(
        default,
        deserialize_with = "crate::plugin_catalog::deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub backend: Option<PluginBackend>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontend: Vec<PluginSurface>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginBackend {
    pub component: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PluginPermission>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPermission {
    WorkspaceContextRead,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSurface {
    pub id: String,
    pub title: String,
    pub document: String,
    pub placement: PluginSurfacePlacement,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PluginPermission>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSurfacePlacement {
    Settings,
    Workspace,
    CompletedVideo,
}

impl PluginCode {
    pub(crate) fn entrypoints(&self) -> impl Iterator<Item = &str> {
        self.backend
            .iter()
            .map(|backend| backend.component.as_str())
            .chain(
                self.frontend
                    .iter()
                    .map(|surface| surface.document.as_str()),
            )
    }
}

pub(crate) fn valid_code_path(path: &str, extension: &str) -> bool {
    path.len() <= 240
        && path.ends_with(extension)
        && path.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginAction {
    pub id: String,
    pub label: String,
    pub description: String,
    pub scope: PluginActionScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginActionScope {
    App,
    Workspace,
}
