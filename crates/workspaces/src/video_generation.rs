//! Generates video clips through the video providers' SDK descriptors: submit a job, poll its
//! status on the same host, then download the finished clip without credentials. Provider
//! specifics live in each plugin's `video-generation.json`.

use std::{
    collections::BTreeMap,
    error::Error,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};

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

const MAX_PROMPT_CHARACTERS: usize = 2500;
const MAX_TEMPLATE_BYTES: usize = 16 * 1024;
const MAX_STATUS_BYTES: u64 = 256 * 1024;

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
    status: StatusMapping,
    poll_seconds: u64,
    timeout_seconds: u64,
    max_video_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Durations {
    min: u32,
    max: u32,
    default: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Submission {
    /// JSON pointer to the job's status address, which must be on the endpoint's host.
    status_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusMapping {
    state: String,
    completed: Vec<String>,
    failed: Vec<String>,
    /// JSON pointer to the finished clip's HTTPS address.
    video: String,
    #[serde(default)]
    error: Option<String>,
}

fn invalid_adapter() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid video adapter")
}

fn host(url: &str) -> Option<&str> {
    url.strip_prefix("https://")?
        .split('/')
        .next()
        .filter(|host| !host.is_empty())
}

fn safe_https(url: &str) -> bool {
    host(url).is_some() && !url.contains(['#', '@', ' ']) && !url.contains("/../")
}

fn pointer(value: &str) -> bool {
    value.starts_with('/') && value.len() <= 128
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
    let durations = &adapter.durations;
    adapter.schema_version == 1
        && adapter.plugin_id == manifest.id
        && manifest.capabilities.contains_key("video-generation")
        && has_secret
        && matches!(adapter.credential_prefix.as_str(), "" | "Bearer " | "Key ")
        && safe_https(&adapter.endpoint)
        && !adapter.endpoint.contains('?')
        && ratios_ok
        && (1..=60).contains(&durations.min)
        && durations.min <= durations.default
        && durations.default <= durations.max
        && durations.max <= 60
        && (2..=60).contains(&adapter.poll_seconds)
        && (60..=3600).contains(&adapter.timeout_seconds)
        && (1..=500 * 1024 * 1024).contains(&adapter.max_video_bytes)
        && pointer(&adapter.submission.status_url)
        && pointer(&adapter.status.state)
        && pointer(&adapter.status.video)
        && adapter.status.error.as_deref().is_none_or(pointer)
        && !adapter.status.completed.is_empty()
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

/// Fills string values that are exactly a placeholder. `{duration}` becomes a number.
fn fill(template: &Value, prompt: &str, ratio: &str, duration: u32) -> Value {
    match template {
        Value::String(text) => match text.as_str() {
            "{prompt}" => Value::String(prompt.to_owned()),
            "{aspectRatio}" => Value::String(ratio.to_owned()),
            "{duration}" => Value::from(duration),
            _ => template.clone(),
        },
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| fill(item, prompt, ratio, duration))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), fill(item, prompt, ratio, duration)))
                .collect(),
        ),
        other => other.clone(),
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

fn read_json(response: reqwest::blocking::Response, what: &str) -> io::Result<Value> {
    if !response.status().is_success() {
        let code = response.status().as_u16();
        let mut text = Vec::new();
        let _ = response.take(4096).read_to_end(&mut text);
        let reason = serde_json::from_slice::<Value>(&text)
            .ok()
            .and_then(|body| {
                ["/detail", "/error", "/message"]
                    .iter()
                    .find_map(|at| body.pointer(at)?.as_str().map(provider_reason))
            })
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        return Err(io::Error::other(format!(
            "The video provider returned HTTP {code} for the {what}{reason}"
        )));
    }
    let mut text = Vec::new();
    response
        .take(MAX_STATUS_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|_| io::Error::other(format!("Could not read the {what}")))?;
    if text.len() as u64 > MAX_STATUS_BYTES {
        return Err(io::Error::other(format!("The {what} is too large")));
    }
    serde_json::from_slice(&text).map_err(|_| io::Error::other(format!("The {what} is not JSON")))
}

/// A provider's short reason, such as `not_enough_credits`, safe to show.
fn provider_reason(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(120).collect()
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
    secret: &[u8],
    prompt: &str,
    ratio: &str,
    duration: u32,
) -> io::Result<(Vec<u8>, &'static str)> {
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| io::Error::other("Could not initialize video transport"))?;
    let body = fill(&adapter.request, prompt, ratio, duration);
    let (header, value) = credential(adapter, secret)?;
    let submitted = read_json(
        client
            .post(&adapter.endpoint)
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
    let status_url = submitted
        .pointer(&adapter.submission.status_url)
        .and_then(Value::as_str)
        .filter(|url| safe_https(url) && host(url) == host(&adapter.endpoint))
        .ok_or_else(|| {
            io::Error::other("The video provider gave no status address on its own host")
        })?
        .to_owned();
    let started = Instant::now();
    let video_url = loop {
        std::thread::sleep(Duration::from_secs(adapter.poll_seconds));
        if started.elapsed() > Duration::from_secs(adapter.timeout_seconds) {
            return Err(io::Error::other(
                "The video provider did not finish in time",
            ));
        }
        let status = read_json(
            client
                .get(&status_url)
                .header(header.clone(), value.clone())
                .send()
                .map_err(|_| io::Error::other("The video status request failed"))?,
            "video status",
        )?;
        let state = status
            .pointer(&adapter.status.state)
            .and_then(Value::as_str)
            .unwrap_or_default();
        if adapter.status.completed.iter().any(|done| done == state) {
            break status
                .pointer(&adapter.status.video)
                .and_then(Value::as_str)
                .filter(|url| safe_https(url))
                .ok_or_else(|| io::Error::other("The finished video has no HTTPS address"))?
                .to_owned();
        }
        if adapter.status.failed.iter().any(|failed| failed == state) {
            let reason = adapter
                .status
                .error
                .as_deref()
                .and_then(|pointer| status.pointer(pointer)?.as_str())
                .map_or_else(|| provider_reason(state), provider_reason);
            return Err(io::Error::other(format!(
                "The video provider stopped: {reason}"
            )));
        }
    };
    // The clip is fetched from wherever the provider stores it, without the credential.
    let download = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(3))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| io::Error::other("Could not initialize video transport"))?
        .get(&video_url)
        .send()
        .map_err(|_| io::Error::other("The finished video could not be downloaded"))?;
    if !download.status().is_success() {
        return Err(io::Error::other(format!(
            "The video download returned HTTP {}",
            download.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    download
        .take(adapter.max_video_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io::Error::other("The finished video could not be downloaded"))?;
    if bytes.is_empty() || bytes.len() as u64 > adapter.max_video_bytes {
        return Err(io::Error::other("The video size is invalid"));
    }
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

impl WorkspaceStore {
    /// Generates a clip with the workspace's first video provider that has an adapter and a key
    /// (or the named `provider`) and adds it to the workspace assets with `notes`. Returns the
    /// asset ID and the provider used.
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
    ) -> Result<(i64, String), Box<dyn Error>> {
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARACTERS || notes.len() > 8000
        {
            return Err(io::Error::other("Describe the clip in up to 2,500 characters").into());
        }
        let order = match provider {
            Some(id) => vec![id.to_owned()],
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
            let Some(ratio) = adapter.aspect_ratios.get(aspect_ratio) else {
                continue;
            };
            let Some(secret) =
                self.provider_secret(plugin, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            let duration = duration
                .unwrap_or(adapter.durations.default)
                .clamp(adapter.durations.min, adapter.durations.max);
            match request_video(&adapter, &secret, prompt, ratio, duration) {
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
                    if !notes.trim().is_empty() {
                        writer.set_asset_notes(workspace_id, asset.id, notes.trim())?;
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
    fn templates_send_the_duration_as_a_number() {
        let body = fill(
            &serde_json::json!({"prompt": "{prompt}", "duration": "{duration}", "aspect_ratio": "{aspectRatio}", "sound": "off"}),
            "Waves",
            "16:9",
            5,
        );
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
                    None
                )
                .is_err()
        );
    }
}
