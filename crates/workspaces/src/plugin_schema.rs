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
    /// For string settings, the only values allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    /// The value used when neither the app nor the workspace sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
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
    /// What the creator fills in (schema 9). The host builds the action's input object from
    /// these, keyed by input ID; without inputs the action takes a raw JSON object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<ActionInput>,
    /// Where the app offers the action besides Settings (schema 9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placements: Vec<ActionPlacement>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionInput {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub kind: ActionInputKind,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

/// How an input is asked for. `subtitles` lets the creator pick a subtitle track of the
/// video in view (or paste or open an SRT file elsewhere) and passes its text; `language` is a
/// language tag picked from a list; `json` is structured data typed or pasted by hand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionInputKind {
    Text,
    LongText,
    Number,
    Boolean,
    Choice,
    Language,
    Subtitles,
    Json,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionPlacement {
    /// Beside a draft while it is being written.
    Draft,
    /// On a video's generation page.
    Generation,
    /// On a finished video.
    CompletedVideo,
}

impl PluginAction {
    pub(crate) fn valid_form(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        let mut places = std::collections::HashSet::new();
        let text =
            |value: &str, max: usize| !value.trim().is_empty() && value.chars().count() <= max;
        self.inputs.len() <= 16
            && self.inputs.iter().all(|input| {
                crate::plugin_catalog::identifier(&input.id)
                    && ids.insert(input.id.as_str())
                    && text(&input.label, 80)
                    && input.description.chars().count() <= 300
                    && if input.kind == ActionInputKind::Choice {
                        (1..=32).contains(&input.choices.len())
                            && input.choices.iter().all(|choice| text(choice, 80))
                    } else {
                        input.choices.is_empty()
                    }
            })
            && self.placements.iter().all(|place| places.insert(*place))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginActionScope {
    App,
    Workspace,
}

#[cfg(test)]
mod action_form_tests {
    use super::*;

    fn action(extra: serde_json::Value) -> PluginAction {
        let mut value = serde_json::json!({
            "id": "translate", "label": "Translate", "description": "Translate subtitles.",
            "scope": "workspace"
        });
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn action_inputs_and_placements_are_checked() {
        let good = action(serde_json::json!({
            "inputs": [
                {"id": "subtitles", "label": "Subtitles", "kind": "subtitles", "required": true},
                {"id": "tone", "label": "Tone", "kind": "choice", "choices": ["formal", "casual"]},
                {"id": "language", "label": "Language", "kind": "language"}
            ],
            "placements": ["completed-video", "draft"]
        }));
        assert!(good.valid_form());
        for bad in [
            serde_json::json!({"inputs": [
                {"id": "a", "label": "A", "kind": "text"}, {"id": "a", "label": "B", "kind": "text"}
            ]}),
            serde_json::json!({"inputs": [{"id": "a", "label": "A", "kind": "choice"}]}),
            serde_json::json!({"inputs": [{"id": "a", "label": "A", "kind": "text", "choices": ["x"]}]}),
            serde_json::json!({"inputs": [{"id": "Bad id", "label": "A", "kind": "text"}]}),
            serde_json::json!({"inputs": [{"id": "a", "label": " ", "kind": "text"}]}),
            serde_json::json!({"placements": ["draft", "draft"]}),
        ] {
            assert!(!action(bad.clone()).valid_form(), "{bad}");
        }
        let unknown = serde_json::from_value::<PluginAction>(serde_json::json!({
            "id": "a", "label": "A", "description": "A", "scope": "app", "placements": ["timeline"]
        }));
        assert!(unknown.is_err(), "unknown places are refused");
    }
}
