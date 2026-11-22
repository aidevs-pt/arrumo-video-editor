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
    /// How to send reference images, for providers that take them.
    #[serde(default)]
    references: Option<ReferenceInput>,
}

/// How a provider receives reference images with a request.
#[derive(Debug, Deserialize)]
#[serde(tag = "encoding", rename_all = "camelCase", deny_unknown_fields)]
enum ReferenceInput {
    /// A multipart form to another endpoint on the same host: the filled request's top-level
    /// fields become form fields and each image a file part named `fileField`.
    #[serde(rename_all = "camelCase")]
    Multipart {
        endpoint: String,
        file_field: String,
        max_images: usize,
    },
    /// The JSON request with `item` (using {mimeType} and {imageBase64}) appended for each image
    /// to the array at `pointer`.
    #[serde(rename_all = "camelCase")]
    Json {
        pointer: String,
        item: Value,
        max_images: usize,
    },
}

/// A generated image stored as a workspace asset.
pub struct GeneratedAsset {
    pub id: i64,
    pub provider: String,
    /// Reference images the provider accepted; 0 when none were sent or it fell back.
    pub references: usize,
}

/// A reference image to send with a request.
pub(crate) struct ReferencePicture {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

fn same_host(first: &str, second: &str) -> bool {
    let host = |url: &str| {
        url.strip_prefix("https://")
            .and_then(|rest| rest.split('/').next())
            .map(str::to_owned)
    };
    host(first).is_some() && host(first) == host(second)
}

fn valid_references(adapter: &ImageAdapter) -> bool {
    match &adapter.references {
        None => true,
        Some(ReferenceInput::Multipart {
            endpoint,
            file_field,
            max_images,
        }) => {
            same_host(endpoint, &adapter.endpoint)
                && !endpoint.contains(['?', '#', '@'])
                && !endpoint.contains("/../")
                && (1..=64).contains(&file_field.len())
                && file_field
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_[]-".contains(c))
                && (1..=8).contains(max_images)
        }
        Some(ReferenceInput::Json {
            pointer,
            item,
            max_images,
        }) => {
            pointer.starts_with('/')
                && pointer.len() <= 256
                && adapter
                    .request
                    .pointer(pointer)
                    .is_some_and(Value::is_array)
                && uses(item, "{imageBase64}")
                && serde_json::to_vec(item).is_ok_and(|bytes| bytes.len() <= 2048)
                && (1..=8).contains(max_images)
        }
    }
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
    /// Reference images the provider accepted with the request.
    pub references: usize,
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
        || !valid_references(&adapter)
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

pub(crate) fn image_extension(bytes: &[u8]) -> Option<&'static str> {
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
        references: &[ReferencePicture],
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
                if let Some(agent) = crate::agent_plugin(provider)
                    .filter(|agent| crate::agent_images::makes_images(agent))
                {
                    match agent_image(agent, prompt, aspect_ratio, references) {
                        Ok(image) => return Ok(image),
                        Err(error) => failure = Some(error),
                    }
                }
                continue;
            };
            // Other ratios are drawn at the nearest one every adapter maps, then cropped.
            let Some(ratio) = adapter.aspect_ratios.get(aspect_ratio).or_else(|| {
                adapter
                    .aspect_ratios
                    .get(nearest_mapped_ratio(aspect_ratio))
            }) else {
                continue;
            };
            let Some(secret) =
                self.provider_secret(provider, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            let model = self.image_model(&adapter, workspace_id);
            // Images are cropped to the exact ratio afterwards, so nothing important may touch the edges.
            let framed = format!(
                "{prompt}\n\nThe final image is cropped to {aspect_ratio}: keep all text and the main subject well inside the frame, away from every edge."
            );
            let attached = match &adapter.references {
                Some(
                    ReferenceInput::Multipart { max_images, .. }
                    | ReferenceInput::Json { max_images, .. },
                ) => &references[..references.len().min(*max_images)],
                None => &[][..],
            };
            let result = if attached.is_empty() {
                request_image(&client, &adapter, &model, ratio, &framed, &secret, &[])
                    .map(|image| (image, 0))
            } else {
                let guided = format!(
                    "{framed}\nThe attached images are the creator's style references: follow their layout, type and colours, but do not copy their text or subjects."
                );
                // A provider that refuses the references still gets a plain request.
                request_image(&client, &adapter, &model, ratio, &guided, &secret, attached)
                    .map(|image| (image, attached.len()))
                    .or_else(|_| {
                        request_image(&client, &adapter, &model, ratio, &framed, &secret, &[])
                            .map(|image| (image, 0))
                    })
            };
            match result {
                Ok(((bytes, extension), references)) => {
                    return Ok(GeneratedImage {
                        provider: provider.clone(),
                        bytes,
                        extension,
                        references,
                    });
                }
                Err(error) => failure = Some(error),
            }
        }
        Err(failure.unwrap_or_else(|| {
            io::Error::other("No image provider for this run could make the image. Sign in to Codex, or add a Gemini or OpenAI key in Settings, Services and keys, then retry")
        }))
    }
}

/// Asks an agent plugin with its own image tool for the image; references go as attached files.
fn agent_image(
    agent: &crate::AgentPlugin,
    prompt: &str,
    aspect_ratio: &str,
    references: &[ReferencePicture],
) -> io::Result<GeneratedImage> {
    let folder = tempfile::tempdir()?;
    let mut paths = Vec::new();
    for (index, reference) in references.iter().enumerate() {
        let path = folder.path().join(format!(
            "reference-{index}.{}",
            reference.mime.trim_start_matches("image/")
        ));
        std::fs::write(&path, &reference.bytes)?;
        paths.push(path);
    }
    let (bytes, extension) = crate::agent_images::generate_with_agent(
        agent,
        prompt,
        aspect_ratio,
        &paths,
        Duration::from_secs(600),
        || Ok(()),
    )
    .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(GeneratedImage {
        provider: agent.id.clone(),
        bytes,
        extension,
        references: if agent
            .cli
            .image_run
            .as_ref()
            .is_some_and(|run| run.args.iter().any(|arg| arg == "{images}"))
        {
            paths.len().min(crate::agent_run::MAX_IMAGES)
        } else {
            0
        },
    })
}

/// The ratio among 16:9, 9:16 and 1:1 (which every adapter maps) closest to `aspect_ratio`.
fn nearest_mapped_ratio(aspect_ratio: &str) -> &'static str {
    let ratio = aspect_ratio
        .split_once(':')
        .and_then(|(width, height)| Some(width.parse::<f64>().ok()? / height.parse::<f64>().ok()?))
        .unwrap_or(1.0);
    if ratio > 1.25 {
        "16:9"
    } else if ratio < 0.75 {
        "9:16"
    } else {
        "1:1"
    }
}

/// A file name from the first words of a description, such as `flat-illustration-of-a-solved`.
fn library_image_stem(prompt: &str) -> String {
    let mut stem = String::new();
    for word in prompt
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        let word: String = word
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        if word.is_empty() {
            continue;
        }
        if stem.len() + word.len() > 40 {
            break;
        }
        if !stem.is_empty() {
            stem.push('-');
        }
        stem.push_str(&word);
    }
    if stem.is_empty() {
        "generated-image".into()
    } else {
        stem
    }
}

/// Aspect ratios the Assets page offers when generating images.
pub const ASSET_ASPECT_RATIOS: [&str; 4] = ["16:9", "9:16", "1:1", "4:5"];

impl WorkspaceStore {
    /// Generates up to four images for the workspace library, from the workspace's image
    /// providers in order or from one chosen `provider`. Each image joins the library with
    /// `notes`, so it is not flagged as missing context. Stops at the first
    /// failure and returns what was made so far with the error.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_library_images(
        &self,
        database: &Path,
        workspace_id: i64,
        prompt: &str,
        aspect_ratio: &str,
        count: usize,
        notes: &str,
        provider: Option<&str>,
    ) -> Result<Vec<GeneratedAsset>, Box<dyn Error>> {
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARACTERS {
            return Err(io::Error::other("Describe the image in up to 4,000 characters").into());
        }
        if !(1..=4).contains(&count)
            || !ASSET_ASPECT_RATIOS.contains(&aspect_ratio)
            || notes.len() > 8000
        {
            return Err(io::Error::other("Choose a size and up to four variations").into());
        }
        let available = self.capture_provider_orders(workspace_id, &BTreeMap::new())?;
        let order = available
            .get("image-generation")
            .map(|order| order.effective.clone())
            .unwrap_or_default();
        let order = match provider {
            Some(id) => {
                let known = self
                    .capability_index(Some(workspace_id))?
                    .get("image-generation")
                    .is_some_and(|providers| providers.iter().any(|entry| entry.id == id));
                if !known {
                    return Err(io::Error::other("That provider does not make images").into());
                }
                vec![id.to_owned()]
            }
            None => order,
        };
        if order.is_empty() {
            return Err(io::Error::other(
                "Turn on an image provider in Settings › Services and keys",
            )
            .into());
        }
        let stem = library_image_stem(prompt);
        let mut made = Vec::new();
        for index in 0..count {
            let image = self.generate_image_asset(
                database,
                workspace_id,
                &order,
                prompt,
                aspect_ratio,
                &if count == 1 {
                    stem.clone()
                } else {
                    format!("{stem}-{}", index + 1)
                },
                &[],
            )?;
            if !notes.trim().is_empty() {
                self.set_asset_notes(workspace_id, image.id, notes.trim())?;
            }
            made.push(image);
        }
        Ok(made)
    }

    /// Generates an image and stores it as a workspace asset named after `stem`.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_image_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        order: &[String],
        prompt: &str,
        aspect_ratio: &str,
        stem: &str,
        reference_ids: &[i64],
    ) -> Result<GeneratedAsset, Box<dyn Error>> {
        let root = crate::asset_root(database);
        let references: Vec<ReferencePicture> = reference_ids
            .iter()
            .filter_map(|id| self.image_preview(&root, workspace_id, *id).ok().flatten())
            .map(|preview| ReferencePicture {
                mime: preview.mime,
                bytes: preview.bytes,
            })
            .collect();
        let image = self.generate_image(workspace_id, order, prompt, aspect_ratio, &references)?;
        let scratch = tempfile::tempdir()?;
        let downloaded = scratch.path().join(format!("{stem}.{}", image.extension));
        std::fs::write(&downloaded, &image.bytes)?;
        let file =
            crop_to_ratio(&downloaded, aspect_ratio, scratch.path(), stem).unwrap_or(downloaded);
        let extension = file
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or(image.extension)
            .to_owned();
        crate::motion_artifacts::validate_motion_file(
            &file,
            &format!("{stem}.{extension}"),
            "image",
        )
        .map_err(|_| io::Error::other("The provider's image could not be verified"))?;
        let mut writer = WorkspaceStore::open(database)?;
        let asset = writer
            .import_asset(&crate::asset_root(database), workspace_id, &file)
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(GeneratedAsset {
            id: asset.id,
            provider: image.provider,
            references: image.references,
        })
    }
}

/// Centre-crops an image to `aspect_ratio` (such as `16:9`) when the provider could only draw a
/// near size, such as 3:2 for wide images. Returns the cropped PNG, or `None` to keep the original.
fn crop_to_ratio(
    source: &Path,
    aspect_ratio: &str,
    folder: &Path,
    stem: &str,
) -> Option<std::path::PathBuf> {
    let (width, height) = aspect_ratio.split_once(':')?;
    let (width, height): (u32, u32) = (width.parse().ok()?, height.parse().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let output = folder.join(format!("{stem}-cropped.png"));
    let ratio = format!("{width}/{height}");
    let status = std::process::Command::new("ffmpeg")
        .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args([
            "-vf",
            &format!("crop='trunc(min(iw,ih*{ratio})/2)*2':'trunc(min(ih,iw/({ratio}))/2)*2'"),
            "-frames:v",
            "1",
        ])
        .arg(&output)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    let cropped = folder.join(format!("{stem}.png"));
    (status.success() && std::fs::rename(&output, &cropped).is_ok()).then_some(cropped)
}

fn request_image(
    client: &Client,
    adapter: &ImageAdapter,
    model: &str,
    ratio: &str,
    prompt: &str,
    secret: &[u8],
    references: &[ReferencePicture],
) -> io::Result<(Vec<u8>, &'static str)> {
    let mut endpoint = adapter.endpoint.replace("{model}", model);
    let (header, value) = credential_header(adapter, secret)?;
    let body = fill(
        &adapter.request,
        &[
            ("{prompt}", prompt),
            ("{aspectRatio}", ratio),
            ("{model}", model),
        ],
    );
    let encode = || io::Error::other("Could not encode the image request");
    let request = match (&adapter.references, references.is_empty()) {
        (
            Some(ReferenceInput::Multipart {
                endpoint: edits,
                file_field,
                ..
            }),
            false,
        ) => {
            endpoint = edits.replace("{model}", model);
            let mut form = reqwest::blocking::multipart::Form::new();
            for (key, value) in body.as_object().into_iter().flatten() {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                form = form.text(key.clone(), text);
            }
            for (index, picture) in references.iter().enumerate() {
                let extension = picture.mime.trim_start_matches("image/");
                let part = reqwest::blocking::multipart::Part::bytes(picture.bytes.clone())
                    .file_name(format!("reference-{}.{extension}", index + 1))
                    .mime_str(picture.mime)
                    .map_err(|_| encode())?;
                form = form.part(file_field.clone(), part);
            }
            client.post(&endpoint).header(header, value).multipart(form)
        }
        (Some(ReferenceInput::Json { pointer, item, .. }), false) => {
            let mut body = body;
            let list = body
                .pointer_mut(pointer)
                .and_then(Value::as_array_mut)
                .ok_or_else(encode)?;
            for picture in references {
                let data = base64::engine::general_purpose::STANDARD.encode(&picture.bytes);
                list.push(fill(
                    item,
                    &[("{mimeType}", picture.mime), ("{imageBase64}", &data)],
                ));
            }
            client
                .post(&endpoint)
                .header(header, value)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(serde_json::to_vec(&body).map_err(|_| encode())?)
        }
        _ => client
            .post(&endpoint)
            .header(header, value)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).map_err(|_| encode())?),
    };
    let response = request
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

    #[test]
    fn unmapped_ratios_are_drawn_at_the_nearest_mapped_one() {
        assert_eq!(nearest_mapped_ratio("4:5"), "1:1");
        assert_eq!(nearest_mapped_ratio("21:9"), "16:9");
        assert_eq!(nearest_mapped_ratio("2:3"), "9:16");
    }

    #[test]
    fn library_images_are_named_from_their_description() {
        assert_eq!(
            library_image_stem("Flat illustration of a solved 3x3 cube, cobalt"),
            "flat-illustration-of-a-solved-3x3-cube"
        );
        assert_eq!(library_image_stem("¡Ñ!"), "generated-image");
    }

    #[test]
    fn library_images_need_a_prompt_a_size_and_one_to_four_variations() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let database = Path::new(":memory:");
        for (prompt, ratio, count) in [
            (" ", "16:9", 1),
            ("Desk", "3:1", 1),
            ("Desk", "16:9", 0),
            ("Desk", "16:9", 5),
        ] {
            assert!(
                store
                    .generate_library_images(database, workspace, prompt, ratio, count, "", None)
                    .is_err()
            );
        }
        assert!(
            store
                .generate_library_images(
                    database,
                    workspace,
                    "Desk",
                    "16:9",
                    1,
                    "",
                    Some("codex-not-a-provider")
                )
                .is_err()
        );
    }
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
    fn wide_images_are_cropped_to_the_requested_ratio() {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("wide.png");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=red:s=1536x1024",
                "-frames:v",
                "1",
            ])
            .arg(&source)
            .status();
        if !made.is_ok_and(|status| status.success()) {
            return;
        }
        let cropped = crop_to_ratio(&source, "16:9", folder.path(), "thumb").unwrap();
        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=width,height",
                "-of",
                "csv=p=0",
            ])
            .arg(&cropped)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&probe.stdout).trim(), "1536,864");
        assert!(crop_to_ratio(&source, "wide", folder.path(), "x").is_none());
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
        assert!(valid_model("gpt-image-2.6-sunburst") && valid_model("gemini-2.5-flash-image"));
        assert!(!valid_model("../x") && !valid_model("a/b") && !valid_model(""));
    }
}
