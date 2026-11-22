//! Generates video clips through the video providers' SDK descriptors: submit a job, poll its
//! status on the same host, then download the finished clip without credentials. Provider
//! specifics live in each plugin's `video-generation.json`.

use std::{collections::BTreeMap, error::Error, io, path::Path, time::Duration};

use reqwest::{
    blocking::Client,
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
    provider_jobs::{JobStatus, Submission, await_output, host, read_json, safe_https},
    provider_media::{MediaKind, UploadDescriptor},
};

const MAX_PROMPT_CHARACTERS: usize = 2500;
const MAX_TEMPLATE_BYTES: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VideoAdapter {
    schema_version: u32,
    plugin_id: String,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    /// Arrumo's ratio (`16:9`, `9:16`, `1:1`) to the value the provider expects.
    aspect_ratios: BTreeMap<String, String>,
    durations: Durations,
    /// The JSON body; string values `{prompt}`, `{aspectRatio}` and `{duration}` (sent as a
    /// number) are filled in.
    request: Value,
    submission: Submission,
    status: JobStatus,
    poll_seconds: u64,
    timeout_seconds: u64,
    max_video_bytes: u64,
    /// Schema 2: which of `models` to use, from a plugin setting or this default.
    #[serde(default)]
    model: Option<ModelChoice>,
    /// Schema 2: each model's request variants, picked by the reference media supplied. Schema 3
    /// models may also carry a label and what they are best for.
    #[serde(default)]
    models: Option<BTreeMap<String, ModelEntry>>,
    /// Schema 2: how reference media reach the provider.
    #[serde(default)]
    upload: Option<UploadDescriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelChoice {
    default: String,
    setting: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ModelEntry {
    Variants(Vec<Variant>),
    #[serde(rename_all = "camelCase")]
    Described {
        label: String,
        best_for: crate::provider_media::Localized,
        variants: Vec<Variant>,
    },
}

impl ModelEntry {
    fn variants(&self) -> &[Variant] {
        match self {
            ModelEntry::Variants(variants) | ModelEntry::Described { variants, .. } => variants,
        }
    }
}

/// One way to call a model, for a number of reference images and videos (none when absent).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Variant {
    endpoint: String,
    /// Absent when the output follows the reference image's shape.
    #[serde(default)]
    aspect_ratios: Option<BTreeMap<String, String>>,
    /// Schema 3: absent when the clip follows its reference video's length.
    #[serde(default)]
    durations: Option<Durations>,
    /// The JSON body; besides the schema 1 placeholders, `{image}` and `{video}` take the first
    /// reference's address, `{lastImage}` the second image and `{images}` and `{videos}` the list.
    request: Value,
    #[serde(default)]
    images: Option<Range>,
    #[serde(default)]
    videos: Option<Range>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    min: usize,
    max: usize,
}

impl Variant {
    fn takes(&self, images: usize, videos: usize) -> bool {
        let fits = |range: &Option<Range>, count: usize| match range {
            Some(range) => (range.min..=range.max).contains(&count),
            None => count == 0,
        };
        fits(&self.images, images) && fits(&self.videos, videos)
    }
}

/// The request to send: the endpoint, ratios, durations and body template for this call.
struct Call<'a> {
    endpoint: &'a str,
    aspect_ratios: Option<&'a BTreeMap<String, String>>,
    durations: Option<&'a Durations>,
    request: &'a Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Durations {
    min: u32,
    max: u32,
    default: u32,
    /// Schema 3: the only lengths the model takes.
    #[serde(default)]
    allowed: Option<Vec<u32>>,
}

impl Durations {
    fn valid(&self, schema_version: u32) -> bool {
        (1..=60).contains(&self.min)
            && self.min <= self.default
            && self.default <= self.max
            && self.max <= 60
            && self.allowed.as_ref().is_none_or(|allowed| {
                schema_version >= 3
                    && (1..=16).contains(&allowed.len())
                    && allowed.contains(&self.default)
                    && allowed
                        .iter()
                        .all(|seconds| (self.min..=self.max).contains(seconds))
            })
    }

    /// The asked length, or the default, moved to the nearest length the model takes.
    fn pick(&self, asked: Option<u32>) -> u32 {
        let asked = asked.unwrap_or(self.default).clamp(self.min, self.max);
        match &self.allowed {
            Some(allowed) => allowed
                .iter()
                .copied()
                .min_by_key(|seconds| (seconds.abs_diff(asked), *seconds))
                .unwrap_or(self.default),
            None => asked,
        }
    }
}

fn invalid_adapter() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid video adapter")
}

fn valid_model_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
}

fn uses(value: &Value, placeholder: &str) -> bool {
    match value {
        Value::String(text) => text == placeholder,
        Value::Array(items) => items.iter().any(|item| uses(item, placeholder)),
        Value::Object(map) => map.values().any(|item| uses(item, placeholder)),
        _ => false,
    }
}

fn valid_variant(variant: &Variant, endpoint: &str, schema_version: u32) -> bool {
    let range_ok = |range: &Option<Range>, single: &str, list: &str, most: usize| match range {
        None => !uses(&variant.request, single) && !uses(&variant.request, list),
        Some(range) => {
            range.min <= range.max
                && range.max <= most
                && range.max > 0
                && (uses(&variant.request, list)
                    || (range.max == 1 && uses(&variant.request, single))
                    || (single == "{image}"
                        && range.max == 2
                        && uses(&variant.request, single)
                        && uses(&variant.request, "{lastImage}")))
        }
    };
    let last_image_ok = !uses(&variant.request, "{lastImage}")
        || schema_version >= 3 && variant.images.as_ref().is_some_and(|range| range.max == 2);
    let durations_ok = match &variant.durations {
        Some(durations) => durations.valid(schema_version),
        None => {
            schema_version >= 3
                && variant.videos.as_ref().is_some_and(|range| range.min > 0)
                && !uses(&variant.request, "{duration}")
        }
    };
    safe_https(&variant.endpoint)
        && host(&variant.endpoint) == host(endpoint)
        && !variant.endpoint.contains('?')
        && variant.aspect_ratios.as_ref().is_none_or(|ratios| {
            !ratios.is_empty()
                && ratios.iter().all(|(ratio, value)| {
                    ["16:9", "9:16", "1:1"].contains(&ratio.as_str())
                        && !value.is_empty()
                        && value.len() <= 32
                })
        })
        && (variant.aspect_ratios.is_some()
            || variant.images.as_ref().is_some_and(|range| range.min > 0)
            || variant.videos.as_ref().is_some_and(|range| range.min > 0))
        && durations_ok
        && last_image_ok
        && uses(&variant.request, "{prompt}")
        && range_ok(
            &variant.images,
            "{image}",
            "{images}",
            crate::provider_media::MAX_REFERENCE_IMAGES,
        )
        && range_ok(
            &variant.videos,
            "{video}",
            "{videos}",
            crate::provider_media::MAX_REFERENCE_VIDEOS,
        )
        && serde_json::to_vec(&variant.request).is_ok_and(|body| body.len() <= MAX_TEMPLATE_BYTES)
}

fn valid_models(adapter: &VideoAdapter, manifest: &crate::PluginManifest) -> bool {
    match (&adapter.model, &adapter.models) {
        (None, None) => adapter.upload.is_none(),
        (Some(choice), Some(models)) => {
            let ids_ok = !models.is_empty()
                && models.len() <= 64
                && models.keys().all(|id| valid_model_id(id))
                && models.contains_key(&choice.default);
            let setting_ok = choice.setting.as_ref().is_none_or(|id| {
                manifest.settings.iter().flatten().any(|setting| {
                    &setting.id == id
                        && setting.kind == PluginSettingKind::String
                        && setting.choices.as_ref().is_some_and(|choices| {
                            choices.iter().all(|choice| models.contains_key(choice))
                        })
                })
            });
            let referenced = models
                .values()
                .flat_map(ModelEntry::variants)
                .any(|variant| variant.images.is_some() || variant.videos.is_some());
            ids_ok
                && setting_ok
                && models.values().all(|model| {
                    let variants = model.variants();
                    let described_ok = match model {
                        ModelEntry::Variants(_) => true,
                        ModelEntry::Described {
                            label, best_for, ..
                        } => {
                            adapter.schema_version >= 3
                                && (1..=64).contains(&label.chars().count())
                                && best_for.valid(240)
                        }
                    };
                    described_ok
                        && !variants.is_empty()
                        && variants.len() <= 4
                        && variants.iter().all(|variant| {
                            valid_variant(variant, &adapter.endpoint, adapter.schema_version)
                        })
                })
                && (!referenced
                    || adapter
                        .upload
                        .as_ref()
                        .is_some_and(|upload| upload.valid(&adapter.endpoint)))
        }
        _ => false,
    }
}

fn valid(adapter: &VideoAdapter, manifest: &crate::PluginManifest) -> bool {
    let has_secret = manifest.settings.iter().flatten().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    let ratios_ok = !adapter.aspect_ratios.is_empty()
        && adapter.aspect_ratios.iter().all(|(ratio, value)| {
            ["16:9", "9:16", "1:1"].contains(&ratio.as_str())
                && !value.is_empty()
                && value.len() <= 32
        });
    (adapter.schema_version == 1 && adapter.model.is_none() && adapter.models.is_none()
        || matches!(adapter.schema_version, 2 | 3) && valid_models(adapter, manifest))
        && adapter.plugin_id == manifest.id
        && manifest.capabilities.contains_key("video-generation")
        && has_secret
        && matches!(adapter.credential_prefix.as_str(), "" | "Bearer " | "Key ")
        && safe_https(&adapter.endpoint)
        && !adapter.endpoint.contains('?')
        && ratios_ok
        && adapter.durations.valid(adapter.schema_version)
        && (2..=60).contains(&adapter.poll_seconds)
        && (60..=3600).contains(&adapter.timeout_seconds)
        && (1..=500 * 1024 * 1024).contains(&adapter.max_video_bytes)
        && adapter.status.valid(&adapter.submission)
        && serde_json::to_vec(&adapter.request).is_ok_and(|body| body.len() <= MAX_TEMPLATE_BYTES)
}

fn video_adapter(plugin_id: &str) -> io::Result<Option<VideoAdapter>> {
    let Some((manifest, source)) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
    else {
        return Ok(None);
    };
    let Some(bytes) = source.video_generation else {
        return Ok(None);
    };
    let adapter: VideoAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    if !valid(&adapter, manifest) {
        return Err(invalid_adapter());
    }
    Ok(Some(adapter))
}

/// What a request template is filled with.
struct Inputs<'a> {
    prompt: &'a str,
    ratio: &'a str,
    duration: Option<u32>,
    images: &'a [String],
    videos: &'a [String],
}

/// Fills string values that are exactly a placeholder. `{duration}` becomes a number and
/// `{images}`/`{videos}` a list. Object fields whose placeholder has no value are left out.
fn fill(template: &Value, inputs: &Inputs) -> Option<Value> {
    let first = |list: &[String]| list.first().map(|url| Value::String(url.clone()));
    let list = |list: &[String]| (!list.is_empty()).then(|| Value::from(list.to_vec()));
    match template {
        Value::String(text) => match text.as_str() {
            "{prompt}" => Some(Value::String(inputs.prompt.to_owned())),
            "{aspectRatio}" => Some(Value::String(inputs.ratio.to_owned())),
            "{duration}" => inputs.duration.map(Value::from),
            "{image}" => first(inputs.images),
            "{lastImage}" => inputs.images.get(1).map(|url| Value::String(url.clone())),
            "{video}" => first(inputs.videos),
            "{images}" => list(inputs.images),
            "{videos}" => list(inputs.videos),
            _ => Some(template.clone()),
        },
        Value::Array(items) => Some(Value::Array(
            items.iter().filter_map(|item| fill(item, inputs)).collect(),
        )),
        Value::Object(map) => Some(Value::Object(
            map.iter()
                .filter_map(|(key, item)| Some((key.clone(), fill(item, inputs)?)))
                .collect(),
        )),
        other => Some(other.clone()),
    }
}

fn credential(adapter: &VideoAdapter, secret: &[u8]) -> io::Result<(HeaderName, HeaderValue)> {
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
        .map_err(|_| io::Error::other("Invalid video credential format"))?;
    value.set_sensitive(true);
    Ok((header, value))
}

/// MP4 and QuickTime files carry an `ftyp` box near the start; WebM starts with the EBML magic.
fn video_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > 12 && &bytes[4..8] == b"ftyp" {
        Some(if &bytes[8..10] == b"qt" { "mov" } else { "mp4" })
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some("webm")
    } else {
        None
    }
}

fn request_video(
    adapter: &VideoAdapter,
    call: &Call,
    secret: &[u8],
    inputs: &Inputs,
) -> io::Result<(Vec<u8>, &'static str)> {
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| io::Error::other("Could not initialize video transport"))?;
    let body = fill(call.request, inputs).unwrap_or(Value::Null);
    let (header, value) = credential(adapter, secret)?;
    let submitted = read_json(
        client
            .post(call.endpoint)
            .header(header.clone(), value.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(
                serde_json::to_vec(&body)
                    .map_err(|_| io::Error::other("Could not encode the video request"))?,
            )
            .send()
            .map_err(|_| io::Error::other("The video request failed"))?,
        "video request",
    )?;
    let bytes = await_output(
        &client,
        &submitted,
        call.endpoint,
        (&header, &value),
        &adapter.submission,
        &adapter.status,
        (adapter.poll_seconds, adapter.timeout_seconds),
        adapter.max_video_bytes,
        "video",
    )?;
    let extension = video_extension(&bytes)
        .ok_or_else(|| io::Error::other("The provider did not return an MP4, MOV or WebM video"))?;
    Ok((bytes, extension))
}

fn clip_stem(prompt: &str) -> String {
    let words: Vec<String> = prompt
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(6)
        .map(str::to_ascii_lowercase)
        .collect();
    if words.is_empty() {
        "generated-clip".into()
    } else {
        format!("clip-{}", words.join("-"))
    }
}

/// A video model a provider offers and the reference media each of its variants takes.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoModel {
    pub provider: String,
    pub id: String,
    /// The model's name; empty when the descriptor gives none.
    pub label: String,
    /// When to choose it, per app language; absent when the descriptor gives none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_for: Option<crate::provider_media::Localized>,
    /// The one used when no model is named: the plugin setting, else the descriptor default.
    pub selected: bool,
    pub variants: Vec<VideoVariant>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoVariant {
    /// Inclusive reference counts; `[0, 0]` takes none.
    pub images: [usize; 2],
    pub videos: [usize; 2],
    /// Both 0 when the clip follows the reference video's length.
    pub min_seconds: u32,
    pub max_seconds: u32,
    /// The only lengths it takes; empty when any length from min to max works.
    pub allowed_seconds: Vec<u32>,
    /// Empty when the output follows the reference image's shape.
    pub aspect_ratios: Vec<String>,
}

impl WorkspaceStore {
    /// The model to use: the caller's, then the plugin setting's, then the descriptor default.
    fn video_model(
        &self,
        adapter: &VideoAdapter,
        workspace_id: i64,
        requested: Option<&str>,
    ) -> Option<String> {
        let choice = adapter.model.as_ref()?;
        let models = adapter.models.as_ref()?;
        requested
            .map(str::to_owned)
            .or_else(|| {
                choice.setting.as_ref().and_then(|id| {
                    self.plugin_settings(&adapter.plugin_id, Some(workspace_id))
                        .ok()?
                        .remove(id)?
                        .effective?
                        .as_str()
                        .map(str::to_owned)
                })
            })
            .filter(|model| models.contains_key(model))
            .or_else(|| Some(choice.default.clone()))
    }

    /// The models of each video provider on for this workspace, in provider order. Providers
    /// with a schema 1 descriptor list one unnamed model.
    pub fn video_models(&self, workspace_id: i64) -> Result<Vec<VideoModel>, Box<dyn Error>> {
        let order = self
            .capture_provider_orders(workspace_id, &BTreeMap::new())?
            .get("video-generation")
            .map(|order| order.effective.clone())
            .unwrap_or_default();
        let range = |range: &Option<Range>| {
            range
                .as_ref()
                .map_or([0, 0], |range| [range.min, range.max])
        };
        let mut listed = Vec::new();
        for plugin in order {
            let Some(adapter) = video_adapter(&plugin)? else {
                continue;
            };
            let selected = self.video_model(&adapter, workspace_id, None);
            match &adapter.models {
                Some(models) => {
                    for (id, model) in models {
                        let (label, best_for) = match model {
                            ModelEntry::Described {
                                label, best_for, ..
                            } => (label.clone(), Some(best_for.clone())),
                            ModelEntry::Variants(_) => (String::new(), None),
                        };
                        listed.push(VideoModel {
                            provider: plugin.clone(),
                            id: id.clone(),
                            label,
                            best_for,
                            selected: selected.as_deref() == Some(id.as_str()),
                            variants: model
                                .variants()
                                .iter()
                                .map(|variant| VideoVariant {
                                    images: range(&variant.images),
                                    videos: range(&variant.videos),
                                    min_seconds: variant.durations.as_ref().map_or(0, |d| d.min),
                                    max_seconds: variant.durations.as_ref().map_or(0, |d| d.max),
                                    allowed_seconds: variant
                                        .durations
                                        .as_ref()
                                        .and_then(|d| d.allowed.clone())
                                        .unwrap_or_default(),
                                    aspect_ratios: variant
                                        .aspect_ratios
                                        .as_ref()
                                        .map(|ratios| ratios.keys().cloned().collect())
                                        .unwrap_or_default(),
                                })
                                .collect(),
                        });
                    }
                }
                None => listed.push(VideoModel {
                    provider: plugin.clone(),
                    id: String::new(),
                    label: String::new(),
                    best_for: None,
                    selected: true,
                    variants: vec![VideoVariant {
                        images: [0, 0],
                        videos: [0, 0],
                        min_seconds: adapter.durations.min,
                        max_seconds: adapter.durations.max,
                        allowed_seconds: adapter.durations.allowed.clone().unwrap_or_default(),
                        aspect_ratios: adapter.aspect_ratios.keys().cloned().collect(),
                    }],
                }),
            }
        }
        Ok(listed)
    }

    /// Generates a clip with the workspace's first video provider that has an adapter and a key
    /// (or the named `provider`) and adds it to the workspace assets with `notes`. `model` picks one
    /// of the provider's models, and `references` are workspace images and MP4 videos sent to it.
    /// Returns the asset ID and the provider used.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_library_video(
        &self,
        database: &Path,
        workspace_id: i64,
        prompt: &str,
        aspect_ratio: &str,
        duration: Option<u32>,
        notes: &str,
        provider: Option<&str>,
        model: Option<&str>,
        references: &[i64],
    ) -> Result<(i64, String), Box<dyn Error>> {
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARACTERS || notes.len() > 8000
        {
            return Err(io::Error::other("Describe the clip in up to 2,500 characters").into());
        }
        if model.is_some() && provider.is_none() {
            return Err(io::Error::other("Name the provider whose model you want").into());
        }
        let media = self.reference_media(database, workspace_id, references)?;
        let images = media
            .iter()
            .filter(|item| item.kind == MediaKind::Image)
            .count();
        let videos = media.len() - images;
        let order = match provider {
            Some(id) => {
                self.check_named_provider(id)?;
                vec![id.to_owned()]
            }
            None => self
                .capture_provider_orders(workspace_id, &BTreeMap::new())?
                .get("video-generation")
                .map(|order| order.effective.clone())
                .unwrap_or_default(),
        };
        let mut failure: Option<Box<dyn Error>> = None;
        for plugin in &order {
            let Some(adapter) = video_adapter(plugin)? else {
                continue;
            };
            let chosen = self.video_model(&adapter, workspace_id, model);
            if let (Some(asked), Some(models)) = (model, &adapter.models)
                && !models.contains_key(asked)
            {
                return Err(io::Error::other(format!(
                    "{plugin} has no model {asked}; it offers {}",
                    models.keys().cloned().collect::<Vec<_>>().join(", ")
                ))
                .into());
            }
            let call = match (&chosen, &adapter.models) {
                (Some(id), Some(models)) => {
                    let Some(variant) = models[id]
                        .variants()
                        .iter()
                        .find(|variant| variant.takes(images, videos))
                    else {
                        let fitting: Vec<_> = models
                            .iter()
                            .filter(|(_, model)| {
                                model
                                    .variants()
                                    .iter()
                                    .any(|variant| variant.takes(images, videos))
                            })
                            .map(|(id, _)| id.clone())
                            .collect();
                        failure = Some(
                            io::Error::other(if fitting.is_empty() {
                                format!("No {plugin} model takes {images} reference images and {videos} reference videos")
                            } else {
                                format!(
                                    "{plugin}'s {id} model can't take {images} reference images and {videos} reference videos; use {}",
                                    fitting.join(" or ")
                                )
                            })
                            .into(),
                        );
                        continue;
                    };
                    Call {
                        endpoint: &variant.endpoint,
                        aspect_ratios: variant.aspect_ratios.as_ref(),
                        durations: variant.durations.as_ref(),
                        request: &variant.request,
                    }
                }
                _ if !media.is_empty() => {
                    failure =
                        Some(io::Error::other(format!("{plugin} takes no reference media")).into());
                    continue;
                }
                _ => Call {
                    endpoint: &adapter.endpoint,
                    aspect_ratios: Some(&adapter.aspect_ratios),
                    durations: Some(&adapter.durations),
                    request: &adapter.request,
                },
            };
            let ratio = match call.aspect_ratios {
                Some(ratios) => match ratios.get(aspect_ratio) {
                    Some(ratio) => ratio.as_str(),
                    None => continue,
                },
                None => aspect_ratio,
            };
            let Some(secret) =
                self.provider_secret(plugin, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            let duration = call.durations.map(|durations| durations.pick(duration));
            let attempt = (|| {
                let (header, value) = credential(&adapter, &secret)?;
                let mut image_urls = Vec::new();
                let mut video_urls = Vec::new();
                for item in &media {
                    let upload = adapter
                        .upload
                        .as_ref()
                        .ok_or_else(|| io::Error::other("This provider takes no uploads"))?;
                    let url = upload.upload((&header, &value), item)?;
                    match item.kind {
                        MediaKind::Image => image_urls.push(url),
                        MediaKind::Video => video_urls.push(url),
                    }
                }
                let inputs = Inputs {
                    prompt,
                    ratio,
                    duration,
                    images: &image_urls,
                    videos: &video_urls,
                };
                request_video(&adapter, &call, &secret, &inputs)
            })();
            match attempt {
                Ok((bytes, extension)) => {
                    let scratch = tempfile::tempdir()?;
                    let name = format!("{}.{extension}", clip_stem(prompt));
                    let file = scratch.path().join(&name);
                    std::fs::write(&file, &bytes)?;
                    crate::motion_artifacts::validate_motion_file(&file, &name, "video").map_err(
                        |_| io::Error::other("The provider's video could not be verified"),
                    )?;
                    let mut writer = WorkspaceStore::open(database)?;
                    let asset = writer
                        .import_asset(&crate::asset_root(database), workspace_id, &file)
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    let made_with = chosen
                        .map(|id| format!("Made with {plugin} {id}."))
                        .unwrap_or_default();
                    let notes = [notes.trim(), made_with.as_str()]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    if !notes.is_empty() {
                        writer.set_asset_notes(workspace_id, asset.id, &notes)?;
                    }
                    return Ok((asset.id, plugin.clone()));
                }
                Err(error) => failure = Some(error.into()),
            }
        }
        Err(failure.unwrap_or_else(|| {
            io::Error::other(
                "Turn on a video provider and add its key in Settings › Services and keys",
            )
            .into()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_higgsfield_adapter_is_valid() {
        let adapter = video_adapter("higgsfield")
            .unwrap()
            .expect("Higgsfield ships a video adapter");
        assert!(adapter.aspect_ratios.contains_key("16:9"));
    }

    #[test]
    fn higgsfield_picks_the_variant_that_takes_the_references() {
        let adapter = video_adapter("higgsfield").unwrap().unwrap();
        let models = adapter.models.as_ref().expect("schema 2 models");
        let pick = |model: &str, images, videos| {
            models[model]
                .variants()
                .iter()
                .find(|variant| variant.takes(images, videos))
                .map(|variant| variant.endpoint.rsplit('/').next().unwrap().to_owned())
        };
        assert_eq!(
            pick("kling-3.0-std", 0, 0).as_deref(),
            Some("text-to-video")
        );
        assert_eq!(
            pick("kling-3.0-pro", 1, 0).as_deref(),
            Some("image-to-video")
        );
        assert_eq!(pick("kling-3.0-std", 0, 1), None);
        assert_eq!(
            pick("kling-3.0-pro", 2, 0).as_deref(),
            Some("image-to-video")
        );
        assert_eq!(pick("seedance-2.0", 0, 0).as_deref(), Some("text-to-video"));
        assert_eq!(
            pick("seedance-2.0", 1, 0).as_deref(),
            Some("image-to-video")
        );
        assert_eq!(pick("seedance-2.0", 3, 0), None);
        assert_eq!(
            pick("seedance-2.0-reference", 3, 1).as_deref(),
            Some("reference-to-video")
        );
        assert_eq!(
            pick("seedance-2.0-reference", 0, 2).as_deref(),
            Some("reference-to-video")
        );
        assert_eq!(pick("kling-3.0-motion-pro", 1, 1).as_deref(), Some("pro"));
        assert_eq!(pick("kling-3.0-motion-pro", 1, 0), None);
        assert_eq!(
            pick("seedance-2.5-extend", 0, 1).as_deref(),
            Some("video-extend")
        );
        for (id, model) in models {
            let ModelEntry::Described {
                label, best_for, ..
            } = model
            else {
                panic!("{id} has no label");
            };
            assert!(!label.is_empty() && best_for.es.is_some() && best_for.pt.is_some());
        }
        assert!(adapter.upload.is_some());
    }

    #[test]
    fn empty_reference_lists_leave_their_field_out() {
        let template = serde_json::json!({"prompt": "{prompt}", "image_urls": "{images}", "video_urls": "{videos}", "image_url": "{image}"});
        let images = vec![
            "https://cdn.example/a.png".to_owned(),
            "https://cdn.example/b.png".to_owned(),
        ];
        let body = fill(
            &template,
            &Inputs {
                prompt: "Waves",
                ratio: "16:9",
                duration: Some(5),
                images: &images,
                videos: &[],
            },
        )
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!({"prompt": "Waves", "image_urls": images, "image_url": "https://cdn.example/a.png"})
        );
    }

    #[test]
    fn variants_need_their_reference_placeholders_and_the_provider_host() {
        let variant = |value: serde_json::Value| serde_json::from_value::<Variant>(value).unwrap();
        let base = "https://api.example.com/model";
        let ok = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/i2v",
            "durations": {"min": 3, "max": 10, "default": 5},
            "request": {"prompt": "{prompt}", "image_url": "{image}"},
            "images": {"min": 1, "max": 1}
        }));
        assert!(valid_variant(&ok, base, 2));
        assert!(!valid_variant(&ok, "https://other.example.com/model", 2));
        let unused = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/i2v",
            "durations": {"min": 3, "max": 10, "default": 5},
            "request": {"prompt": "{prompt}"},
            "images": {"min": 1, "max": 4}
        }));
        assert!(!valid_variant(&unused, base, 2));
        let stray = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/t2v",
            "aspectRatios": {"16:9": "16:9"},
            "durations": {"min": 3, "max": 10, "default": 5},
            "request": {"prompt": "{prompt}", "video_urls": "{videos}"}
        }));
        assert!(!valid_variant(&stray, base, 2));
        let frames = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/i2v",
            "durations": {"min": 3, "max": 10, "default": 5},
            "request": {"prompt": "{prompt}", "image_url": "{image}", "last_image_url": "{lastImage}"},
            "images": {"min": 1, "max": 2}
        }));
        assert!(valid_variant(&frames, base, 3));
        assert!(!valid_variant(&frames, base, 2));
        let edit = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/edit",
            "request": {"prompt": "{prompt}", "video_url": "{video}"},
            "videos": {"min": 1, "max": 1}
        }));
        assert!(valid_variant(&edit, base, 3));
        assert!(!valid_variant(&edit, base, 2));
        let timed_edit = variant(serde_json::json!({
            "endpoint": "https://api.example.com/model/edit",
            "request": {"prompt": "{prompt}", "video_url": "{video}", "duration": "{duration}"},
            "videos": {"min": 1, "max": 1}
        }));
        assert!(!valid_variant(&timed_edit, base, 3));
    }

    #[test]
    fn lengths_move_to_the_nearest_one_the_model_takes() {
        let durations: Durations = serde_json::from_value(
            serde_json::json!({"min": 5, "max": 10, "default": 5, "allowed": [5, 10]}),
        )
        .unwrap();
        assert!(durations.valid(3));
        assert!(!durations.valid(2));
        assert_eq!(durations.pick(None), 5);
        assert_eq!(durations.pick(Some(8)), 10);
        assert_eq!(durations.pick(Some(7)), 5);
        assert_eq!(durations.pick(Some(30)), 10);
        let open: Durations =
            serde_json::from_value(serde_json::json!({"min": 3, "max": 15, "default": 5})).unwrap();
        assert_eq!(open.pick(Some(8)), 8);
        assert_eq!(open.pick(Some(1)), 3);
    }

    #[test]
    fn templates_send_the_duration_as_a_number() {
        let body = fill(
            &serde_json::json!({"prompt": "{prompt}", "duration": "{duration}", "aspect_ratio": "{aspectRatio}", "sound": "off", "last_image_url": "{video}"}),
            &Inputs { prompt: "Waves", ratio: "16:9", duration: Some(5), images: &[], videos: &[] },
        )
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!({"prompt": "Waves", "duration": 5, "aspect_ratio": "16:9", "sound": "off"})
        );
    }

    #[test]
    fn status_addresses_must_be_https() {
        assert!(safe_https("https://api.example.com/requests/1/status"));
        assert!(!safe_https("http://api.example.com/x"));
        assert!(!safe_https("https://user@evil.example/x"));
        assert_eq!(host("https://api.example.com/a"), Some("api.example.com"));
    }

    #[test]
    fn recognises_video_files() {
        assert_eq!(video_extension(b"\0\0\0\x18ftypisom\0\0\0\0"), Some("mp4"));
        assert_eq!(video_extension(b"\0\0\0\x14ftypqt  \0\0\0\0"), Some("mov"));
        assert_eq!(video_extension(b"<html>not a video</html>"), None);
        assert_eq!(
            clip_stem("Waves at sunset, slow pan!"),
            "clip-waves-at-sunset-slow-pan"
        );
    }

    #[test]
    fn library_clips_need_a_description() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        assert!(
            store
                .generate_library_video(
                    Path::new(":memory:"),
                    workspace,
                    " ",
                    "16:9",
                    None,
                    "",
                    None,
                    None,
                    &[]
                )
                .is_err()
        );
    }
}
