//! What a plugin says about itself in the creator's language, and the icon it is shown with.
//! Plugins own these: English text stays in the manifest's own fields, and `translations`
//! override it for other languages. The icon is a small SVG shown as an image, never inlined
//! into the page, so it cannot run script or load anything.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::plugin_catalog::PluginManifest;

pub const TRANSLATED_LANGUAGES: [&str; 3] = ["en", "es", "pt"];
const MAX_ICON_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginTranslation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderTranslation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, NamedText>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, SettingText>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, LabelledText>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTranslation {
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedText {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabelledText {
    pub label: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Labels for the setting's choices, keyed by the choice value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub choices: BTreeMap<String, String>,
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= limit
}

/// Whether every translation names a language Arrumo speaks and something the manifest declares.
pub(crate) fn valid_translations(manifest: &PluginManifest) -> bool {
    let Some(translations) = &manifest.translations else {
        return true;
    };
    translations.iter().all(|(language, entry)| {
        TRANSLATED_LANGUAGES.contains(&language.as_str())
            && entry.name.as_deref().is_none_or(|name| text(name, 80))
            && entry
                .description
                .as_deref()
                .is_none_or(|value| text(value, 500))
            && entry.provider.as_ref().is_none_or(|provider| {
                manifest.agent_provider.is_some() && text(&provider.label, 80)
            })
            && entry.capabilities.iter().all(|(id, value)| {
                manifest.capabilities.contains_key(id)
                    && text(&value.name, 80)
                    && text(&value.description, 500)
            })
            && entry.actions.iter().all(|(id, value)| {
                manifest
                    .actions
                    .iter()
                    .flatten()
                    .any(|action| &action.id == id)
                    && text(&value.label, 80)
                    && text(&value.description, 500)
            })
            && entry.settings.iter().all(|(id, value)| {
                manifest
                    .settings
                    .iter()
                    .flatten()
                    .find(|setting| &setting.id == id)
                    .is_some_and(|setting| {
                        value.label.as_deref().is_none_or(|label| text(label, 80))
                            && value
                                .description
                                .as_deref()
                                .is_none_or(|value| text(value, 500))
                            && value.choices.iter().all(|(choice, label)| {
                                setting
                                    .choices
                                    .iter()
                                    .flatten()
                                    .any(|known| known == choice)
                                    && text(label, 80)
                            })
                    })
            })
    })
}

/// A small, self-contained SVG: no script, event handlers, external references or embedded
/// documents.
pub(crate) fn valid_icon(svg: &str) -> bool {
    let lower = svg.to_ascii_lowercase();
    let trimmed = lower.trim_start();
    svg.len() <= MAX_ICON_BYTES
        && trimmed.starts_with("<svg")
        && lower.trim_end().ends_with("</svg>")
        && ![
            "<script",
            "<foreignobject",
            "<iframe",
            "<image",
            "<use",
            "javascript:",
            "data:",
            "@import",
            "url(",
            "<!entity",
            "<!doctype",
            "href",
        ]
        .iter()
        .any(|banned| lower.contains(banned))
        && !lower
            .split(|c: char| c.is_whitespace() || c == '<')
            .any(|token| token.starts_with("on") && token.contains('='))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_must_be_inert_svg() {
        assert!(valid_icon(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4h16v16H4z"/></svg>"#
        ));
        for bad in [
            r#"<svg><script>alert(1)</script></svg>"#,
            r#"<svg onload="x()"><path/></svg>"#,
            r#"<svg><image href="https://x/y.png"/></svg>"#,
            r##"<svg><use xlink:href="#a"/></svg>"##,
            r#"<svg><rect style="fill:url(https://x)"/></svg>"#,
            "<html></html>",
        ] {
            assert!(!valid_icon(bad), "{bad}");
        }
    }

    #[test]
    fn translations_must_match_what_the_manifest_declares() {
        for id in ["codex", "claude-code"] {
            let plugin = crate::agent_plugin(id).unwrap();
            let manifest = crate::plugin_catalog::bundled_manifests()
                .iter()
                .find(|manifest| manifest.id == plugin.id)
                .unwrap();
            assert!(valid_translations(manifest));
            assert!(manifest.icon.as_deref().is_some_and(valid_icon), "{id}");
            let mut broken = manifest.clone();
            broken
                .translations
                .get_or_insert_default()
                .entry("es".into())
                .or_default()
                .settings
                .insert(
                    "missing-setting".into(),
                    SettingText {
                        label: Some("x".into()),
                        description: None,
                        choices: BTreeMap::new(),
                    },
                );
            assert!(!valid_translations(&broken));
            let mut foreign = manifest.clone();
            foreign
                .translations
                .get_or_insert_default()
                .insert("fr".into(), PluginTranslation::default());
            assert!(!valid_translations(&foreign));
        }
    }
}
