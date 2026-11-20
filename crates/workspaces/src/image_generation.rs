//! Generates images through the image providers' SDK adapters, in the run's captured order, and
//! stores each image as a workspace asset. Provider specifics live in the plugins' descriptors.

use std::{collections::BTreeMap, error::Error, io, io::Read, path::Path, time::Duration};

use base64::Engine;
use reqwest::{
    blocking::Client,
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
};

const MAX_PROMPT_CHARACTERS: usize = 4000;
const MAX_TEMPLATE_BYTES: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageAdapter {
    schema_version: u32,
    plugin_id: String,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    model: ModelChoice,
    aspect_ratios: BTreeMap<String, String>,
    request: Value,
    response: ImageResponse,
    max_image_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelChoice {
    default: String,
    setting: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageResponse {
    items: Option<String>,
    image: String,
}

/// A generated image, before it is stored.
#[derive(Debug)]
pub(crate) struct GeneratedImage {
    pub provider: String,
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

fn invalid_adapter() -> io::Error {
    io::Error::other("Invalid image generation adapter")
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 128
        && model.starts_with(|c: char| c.is_ascii_alphanumeric())
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn image_adapter(plugin_id: &str) -> io::Result<Option<ImageAdapter>> {
    let Some((manifest, source)) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
    else {
        return Ok(None);
    };
    let Some(bytes) = source.image_generation else {
        return Ok(None);
    };
    let adapter: ImageAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    let settings = manifest.settings.iter().flatten();
    let has_secret = settings.clone().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    let model_setting_ok = adapter.model.setting.as_ref().is_none_or(|id| {
        settings
            .clone()
            .any(|setting| &setting.id == id && setting.kind == PluginSettingKind::String)
    });
    if adapter.schema_version != 1
        || adapter.plugin_id != manifest.id
        || !manifest.capabilities.contains_key("image-generation")
        || !has_secret
        || !model_setting_ok
        || !valid_model(&adapter.model.default)
        || !matches!(adapter.credential_prefix.as_str(), "" | "Bearer ")
        || !(1..=20 * 1024 * 1024).contains(&adapter.max_image_bytes)
        || !["16:9", "9:16", "1:1"]
            .iter()
            .all(|ratio| adapter.aspect_ratios.contains_key(*ratio))
        || !adapter.endpoint.starts_with("https://")
        || adapter.endpoint.contains(['?', '#', '@'])
        || adapter.endpoint.contains("/../")
        || serde_json::to_vec(&adapter.request).map_or(true, |body| body.len() > MAX_TEMPLATE_BYTES)
        || !uses(&adapter.request, "{prompt}")
    {
        return Err(invalid_adapter());
    }
    Ok(Some(adapter))
}

fn uses(value: &Value, placeholder: &str) -> bool {
    match value {
        Value::String(text) => text == placeholder,
        Value::Array(items) => items.iter().any(|item| uses(item, placeholder)),
        Value::Object(map) => map.values().any(|item| uses(item, placeholder)),
        _ => false,
    }
}

/// Replaces string values that are exactly a placeholder; other strings are left as they are.
fn fill(template: &Value, values: &[(&str, &str)]) -> Value {
    match template {
        Value::String(text) => values
            .iter()
            .find(|(placeholder, _)| placeholder == text)
            .map_or_else(
                || template.clone(),
                |(_, value)| Value::String((*value).to_owned()),
            ),
        Value::Array(items) => Value::Array(items.iter().map(|item| fill(item, values)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), fill(item, values)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn image_data<'a>(response: &'a Value, mapping: &ImageResponse) -> Option<&'a str> {
    match &mapping.items {
        Some(items) => response
            .pointer(items)?
            .as_array()?
            .iter()
            .find_map(|item| item.pointer(&mapping.image)?.as_str()),
        None => response.pointer(&mapping.image)?.as_str(),
    }
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn credential_header(
    adapter: &ImageAdapter,
    secret: &[u8],
) -> io::Result<(HeaderName, HeaderValue)> {
    let header = HeaderName::from_bytes(adapter.credential_header.as_bytes())
        .map_err(|_| invalid_adapter())?;
    if matches!(
        header.as_str(),
        "host" | "content-type" | "content-length" | "transfer-encoding" | "connection"
    ) {
        return Err(invalid_adapter());
    }
    let mut bytes = zeroize::Zeroizing::new(adapter.credential_prefix.as_bytes().to_vec());
    bytes.extend_from_slice(secret);
    let mut value = HeaderValue::from_bytes(&bytes)
        .map_err(|_| io::Error::other("Invalid image credential format"))?;
    value.set_sensitive(true);
    Ok((header, value))
}

impl WorkspaceStore {
    fn image_model(&self, adapter: &ImageAdapter, workspace_id: i64) -> String {
        adapter
            .model
            .setting
            .as_ref()
            .and_then(|id| {
                self.plugin_settings(&adapter.plugin_id, Some(workspace_id))
                    .ok()?
                    .remove(id)
            })
            .and_then(|setting| setting.effective)
            .and_then(|value| value.as_str().map(str::trim).map(str::to_owned))
            .filter(|model| valid_model(model))
            .unwrap_or_else(|| adapter.model.default.clone())
    }

    /// Asks each provider in `order` until one returns an image. Providers without an adapter or
    /// a configured key are skipped.
    pub(crate) fn generate_image(
        &self,
        workspace_id: i64,
        order: &[String],
        prompt: &str,
        aspect_ratio: &str,
    ) -> io::Result<GeneratedImage> {
        let prompt = prompt.trim();
        if prompt.is_empty()
            || prompt.contains('\0')
            || prompt.chars().count() > MAX_PROMPT_CHARACTERS
        {
            return Err(io::Error::other("Invalid image prompt"));
        }
        let client = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|_| io::Error::other("Could not initialize image transport"))?;
        let mut failure = None;
        for provider in order {
            let Some(adapter) = image_adapter(provider)? else {
                continue;
            };
            let Some(ratio) = adapter.aspect_ratios.get(aspect_ratio) else {
                continue;
            };
            let Some(secret) =
                self.provider_secret(provider, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            let model = self.image_model(&adapter, workspace_id);
            match request_image(&client, &adapter, &model, ratio, prompt, &secret) {
                Ok((bytes, extension)) => {
                    return Ok(GeneratedImage {
                        provider: provider.clone(),
                        bytes,
                        extension,
                    });
                }
                Err(error) => failure = Some(error),
            }
        }
        Err(failure.unwrap_or_else(|| {
            io::Error::other("No image provider for this run has a key. Add a Gemini or OpenAI key in Settings, Services and keys, then retry")
        }))
    }

    /// Generates an image and stores it as a workspace asset named after `stem`.
    pub fn generate_image_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        order: &[String],
        prompt: &str,
        aspect_ratio: &str,
        stem: &str,
    ) -> Result<(i64, String), Box<dyn Error>> {
        let image = self.generate_image(workspace_id, order, prompt, aspect_ratio)?;
        let scratch = tempfile::tempdir()?;
        let file = scratch.path().join(format!("{stem}.{}", image.extension));
        std::fs::write(&file, &image.bytes)?;
        crate::motion_artifacts::validate_motion_file(
            &file,
            &format!("{stem}.{}", image.extension),
            "image",
        )
        .map_err(|_| io::Error::other("The provider's image could not be verified"))?;
        let mut writer = WorkspaceStore::open(database)?;
        let asset = writer
            .import_asset(&crate::asset_root(database), workspace_id, &file)
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok((asset.id, image.provider))
    }
}

fn request_image(
    client: &Client,
    adapter: &ImageAdapter,
    model: &str,
    ratio: &str,
    prompt: &str,
    secret: &[u8],
) -> io::Result<(Vec<u8>, &'static str)> {
    let endpoint = adapter.endpoint.replace("{model}", model);
    let (header, value) = credential_header(adapter, secret)?;
    let body = fill(
        &adapter.request,
        &[
            ("{prompt}", prompt),
            ("{aspectRatio}", ratio),
            ("{model}", model),
        ],
    );
    let response = client
        .post(&endpoint)
        .header(header, value)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::to_vec(&body)
                .map_err(|_| io::Error::other("Could not encode the image request"))?,
        )
        .send()
        .map_err(|_| io::Error::other("The image request failed"))?;
    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "The image provider returned HTTP {}",
            response.status().as_u16()
        )));
    }
    let limit = adapter.max_image_bytes * 4 / 3 + 1024 * 1024;
    let mut text = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut text)
        .map_err(|_| io::Error::other("Could not read the image response"))?;
    if text.len() as u64 > limit {
        return Err(io::Error::other("The image response is too large"));
    }
    let json: Value = serde_json::from_slice(&text)
        .map_err(|_| io::Error::other("The image response is not JSON"))?;
    let data = image_data(&json, &adapter.response)
        .ok_or_else(|| io::Error::other("The image response has no image"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| io::Error::other("The image data is not valid base64"))?;
    if bytes.is_empty() || bytes.len() as u64 > adapter.max_image_bytes {
        return Err(io::Error::other("The image size is invalid"));
    }
    let extension = image_extension(&bytes)
        .ok_or_else(|| io::Error::other("The provider did not return a PNG, JPEG or WebP image"))?;
    Ok((bytes, extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bundled_image_adapters_are_valid() {
        for id in ["openai", "gemini"] {
            let adapter = image_adapter(id).unwrap().expect("bundled image adapter");
            assert_eq!(adapter.plugin_id, id);
            assert!(adapter.model.setting.is_some());
        }
    }

    #[test]
    fn templates_replace_whole_placeholders_only() {
        let template = json!({"contents": [{"parts": [{"text": "{prompt}"}]}], "note": "keep {prompt}", "size": "{aspectRatio}", "n": 1});
        let body = fill(
            &template,
            &[("{prompt}", "A red build"), ("{aspectRatio}", "16:9")],
        );
        assert_eq!(
            body,
            json!({"contents": [{"parts": [{"text": "A red build"}]}], "note": "keep {prompt}", "size": "16:9", "n": 1})
        );
        assert!(uses(&template, "{prompt}") && !uses(&json!({"text": "say {prompt}"}), "{prompt}"));
    }

    #[test]
    fn responses_find_the_first_item_with_an_image() {
        let mapping = ImageResponse {
            items: Some("/candidates/0/content/parts".into()),
            image: "/inlineData/data".into(),
        };
        let response = json!({"candidates": [{"content": {"parts": [{"text": "Here"}, {"inlineData": {"data": "aGk="}}]}}]});
        assert_eq!(image_data(&response, &mapping), Some("aGk="));
        let flat = ImageResponse {
            items: None,
            image: "/data/0/b64_json".into(),
        };
        assert_eq!(
            image_data(&json!({"data": [{"b64_json": "eA=="}]}), &flat),
            Some("eA==")
        );
        assert_eq!(image_data(&json!({"data": []}), &flat), None);
    }

    #[test]
    fn only_real_image_formats_and_safe_models_are_accepted() {
        assert_eq!(image_extension(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(image_extension(&[0xff, 0xd8, 0xff, 0xe0]), Some("jpg"));
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_extension(b"<svg>"), None);
        assert!(valid_model("gpt-image-2.5-flare") && valid_model("gemini-2.5-flash-image"));
        assert!(!valid_model("../x") && !valid_model("a/b") && !valid_model(""));
    }
}
