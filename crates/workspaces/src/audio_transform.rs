//! Voice changing and voice isolation by a trusted provider plugin's `audio-transform.json`
//! descriptor: a workspace audio or video asset goes in, a new audio asset comes out.

use crate::Launch;
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::Path,
    process::Stdio,
    time::Duration,
};

use reqwest::{
    Url,
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
    speech_synthesis::{speech_client_builder, valid_key},
};

const MAX_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;

/// What a provider does to the audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioTransform {
    /// Says the same words in another voice.
    VoiceChange,
    /// Keeps the voice and removes the rest.
    VoiceIsolation,
}

impl AudioTransform {
    fn key(self) -> &'static str {
        match self {
            Self::VoiceChange => "voice-change",
            Self::VoiceIsolation => "voice-isolation",
        }
    }

    fn capability(self) -> &'static str {
        match self {
            Self::VoiceChange => "voice-changer",
            Self::VoiceIsolation => "voice-isolation",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::VoiceChange => "Changed voice",
            Self::VoiceIsolation => "Isolated voice",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::VoiceChange => "voice-changed",
            Self::VoiceIsolation => "voice-isolated",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransformAdapter {
    schema_version: u32,
    plugin_id: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    #[serde(default)]
    voice_setting: Option<String>,
    transforms: BTreeMap<String, TransformEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransformEntry {
    endpoint: String,
    voice_segment: bool,
    file_field: String,
    fields: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
    max_input_seconds: u32,
    max_input_bytes: u64,
    #[serde(default)]
    min_input_seconds: Option<f64>,
}

fn invalid_adapter() -> io::Error {
    io::Error::other("Invalid audio transform adapter")
}

pub(crate) fn valid_endpoint(endpoint: &str) -> bool {
    let Some((host, path)) = endpoint
        .strip_prefix("https://")
        .and_then(|raw| raw.split_once('/'))
    else {
        return false;
    };
    endpoint.len() <= 2048
        && !host.is_empty()
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        && path.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._~-".contains(&byte))
        })
}

fn valid_entry(entry: &TransformEntry) -> bool {
    let pairs_ok = |map: &BTreeMap<String, String>| {
        map.len() <= 16
            && map
                .iter()
                .all(|(key, value)| valid_key(key) && value.chars().count() <= 256)
    };
    valid_endpoint(&entry.endpoint)
        && valid_key(&entry.file_field)
        && !entry.fields.contains_key(&entry.file_field)
        && pairs_ok(&entry.fields)
        && pairs_ok(&entry.query)
        && (1..=7200).contains(&entry.max_input_seconds)
        && entry
            .min_input_seconds
            .is_none_or(|min| min > 0.0 && min <= 600.0 && min < f64::from(entry.max_input_seconds))
        && (1..=1024 * 1024 * 1024).contains(&entry.max_input_bytes)
}

fn transform_adapter(plugin_id: &str) -> io::Result<Option<TransformAdapter>> {
    let Some((manifest, source)) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
    else {
        return Ok(None);
    };
    let Some(bytes) = source.audio_transform else {
        return Ok(None);
    };
    let adapter: TransformAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    let settings = manifest.settings.iter().flatten();
    let has_secret = settings.clone().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    let voice_ok = adapter.voice_setting.as_ref().is_none_or(|id| {
        settings
            .clone()
            .any(|setting| &setting.id == id && setting.kind == PluginSettingKind::String)
    });
    let transforms_ok = !adapter.transforms.is_empty()
        && adapter.transforms.iter().all(|(key, entry)| {
            let capability = match key.as_str() {
                "voice-change" => AudioTransform::VoiceChange,
                "voice-isolation" => AudioTransform::VoiceIsolation,
                _ => return false,
            }
            .capability();
            manifest.capabilities.contains_key(capability)
                && valid_entry(entry)
                && (key != "voice-change" || entry.voice_segment && adapter.voice_setting.is_some())
        });
    if adapter.schema_version != 1
        || adapter.plugin_id != manifest.id
        || !has_secret
        || !voice_ok
        || !transforms_ok
        || !matches!(adapter.credential_prefix.as_str(), "" | "Bearer ")
    {
        return Err(invalid_adapter());
    }
    Ok(Some(adapter))
}

/// The audio container in `bytes`, from its signature.
pub(crate) fn audio_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"ID3") || bytes.len() > 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0 {
        Some("mp3")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Some("wav")
    } else if bytes.starts_with(b"fLaC") {
        Some("flac")
    } else if bytes.starts_with(b"OggS") {
        Some("ogg")
    } else {
        None
    }
}

/// Converts a workspace media file to an MP3 of its soundtrack for upload.
pub(crate) fn soundtrack_mp3(source: &Path, output: &Path) -> io::Result<()> {
    let status = crate::quiet_command(crate::media_tools::ffmpeg())
        .args(["-nostdin", "-v", "error", "-y", "-i"])
        .arg(source)
        .args([
            "-vn",
            "-map",
            "0:a:0",
            "-ac",
            "2",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "2",
        ])
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .launch_status()?;
    if !status.success() {
        return Err(io::Error::other(
            "The asset has no sound Arrumo can read, so there is nothing to process",
        ));
    }
    Ok(())
}

/// The provider's own short explanation of a refused request, from `detail.message`,
/// `detail` or `message` in a JSON error body.
pub(crate) fn provider_reason(response: reqwest::blocking::Response) -> Option<String> {
    let mut body = Vec::new();
    response.take(16 * 1024).read_to_end(&mut body).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&body).ok()?;
    let text = value
        .pointer("/detail/message")
        .or_else(|| value.get("detail"))
        .or_else(|| value.get("message"))?
        .as_str()?;
    let clean: String = text.chars().filter(|c| !c.is_control()).take(240).collect();
    (!clean.trim().is_empty()).then(|| clean.trim().to_owned())
}

/// Names the result after its source: `interview.mp4` becomes `interview-voice-isolated.mp3`.
fn result_stem(source_name: &str, transform: AudioTransform) -> String {
    let stem = Path::new(source_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("audio")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "audio" } else { stem };
    format!("{}-{}", &stem[..stem.len().min(60)], transform.suffix())
}

/// The providers on for this workspace that offer `transform`, in provider order.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioTransformProvider {
    pub provider: String,
    pub transform: String,
    pub min_input_seconds: Option<f64>,
    pub max_input_seconds: u32,
}

impl WorkspaceStore {
    /// Which audio transforms each provider offers in this workspace.
    pub fn audio_transform_providers(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<AudioTransformProvider>, Box<dyn std::error::Error>> {
        let index = self.capability_index(Some(workspace_id))?;
        let mut listed = Vec::new();
        for transform in [AudioTransform::VoiceChange, AudioTransform::VoiceIsolation] {
            for entry in index.get(transform.capability()).into_iter().flatten() {
                if let Some(adapter) = transform_adapter(&entry.id)?
                    && let Some(spec) = adapter.transforms.get(transform.key())
                {
                    listed.push(AudioTransformProvider {
                        provider: entry.id.clone(),
                        transform: transform.key().to_owned(),
                        min_input_seconds: spec.min_input_seconds,
                        max_input_seconds: spec.max_input_seconds,
                    });
                }
            }
        }
        Ok(listed)
    }

    /// Sends the sound of workspace asset `asset_id` through `transform` with the first provider
    /// that offers it (or `provider`) and adds the result as a new labelled audio asset whose notes
    /// name the source. `voice` overrides the provider's configured voice for a voice change.
    /// Returns the new asset ID and the provider used.
    #[allow(clippy::too_many_arguments)]
    pub fn transform_audio_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        transform: AudioTransform,
        provider: Option<&str>,
        voice: Option<&str>,
    ) -> Result<(i64, String), Box<dyn std::error::Error>> {
        let source = self
            .list_assets(workspace_id)?
            .into_iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| io::Error::other("That asset is not in this workspace"))?;
        if !matches!(source.kind.as_str(), "audio" | "video") {
            return Err(io::Error::other("Choose an audio or video asset").into());
        }
        if let Some(id) = provider {
            self.check_named_provider(id)?;
        }
        let offered: Vec<String> = self
            .capability_index(Some(workspace_id))?
            .get(transform.capability())
            .map(|providers| providers.iter().map(|entry| entry.id.clone()).collect())
            .unwrap_or_default();
        let plugin = match provider {
            Some(id) if offered.iter().any(|entry| entry == id) => id.to_owned(),
            Some(_) => {
                return Err(io::Error::other("That provider does not offer this").into());
            }
            None => self
                .capture_provider_orders(workspace_id, &BTreeMap::new())?
                .get(transform.capability())
                .and_then(|order| order.effective.first().cloned())
                .or_else(|| offered.first().cloned())
                .ok_or_else(|| {
                    io::Error::other("Turn on a provider for this in Settings › Services and keys")
                })?,
        };
        let adapter = transform_adapter(&plugin)?
            .ok_or_else(|| io::Error::other("That provider does not offer this"))?;
        let entry = adapter
            .transforms
            .get(transform.key())
            .ok_or_else(|| io::Error::other("That provider does not offer this"))?;
        let secret = self
            .provider_secret(&plugin, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| {
                io::Error::other("Add the provider's key in Settings › Services and keys")
            })?;
        let voice = match transform {
            AudioTransform::VoiceChange => Some(match voice.map(str::trim) {
                Some(voice) if !voice.is_empty() => voice.to_owned(),
                _ => adapter
                    .voice_setting
                    .as_ref()
                    .and_then(|id| {
                        self.plugin_settings(&plugin, Some(workspace_id))
                            .ok()?
                            .remove(id)?
                            .effective?
                            .as_str()
                            .map(|voice| voice.trim().to_owned())
                    })
                    .filter(|voice| !voice.is_empty())
                    .or(self.narration_voice(workspace_id, &plugin).ok().flatten())
                    .ok_or_else(|| io::Error::other("Choose a voice in the plugin's settings"))?,
            }),
            AudioTransform::VoiceIsolation => None,
        };
        if voice.as_ref().is_some_and(|voice| {
            voice.len() > 256
                || matches!(voice.as_str(), "." | "..")
                || voice.chars().any(|c| c.is_control() || c == '/')
        }) {
            return Err(io::Error::other("Invalid voice identifier").into());
        }

        let scratch = tempfile::tempdir()?;
        let input = scratch.path().join("source.mp3");
        let original = crate::asset_path(&crate::asset_root(database), workspace_id, asset_id);
        soundtrack_mp3(&original, &input)?;
        let seconds = crate::source_transcripts::media_duration(&input)?;
        if seconds > f64::from(entry.max_input_seconds) {
            return Err(io::Error::other(format!(
                "This provider takes up to {} seconds; the sound is {} seconds long. Trim it first",
                entry.max_input_seconds,
                seconds.ceil()
            ))
            .into());
        }
        if let Some(min) = entry.min_input_seconds
            && seconds < min
        {
            return Err(io::Error::other(format!(
                "This provider needs at least {min} seconds of sound; this is {:.1} seconds long",
                seconds
            ))
            .into());
        }
        let bytes = std::fs::read(&input)?;
        if bytes.len() as u64 > entry.max_input_bytes {
            return Err(io::Error::other("The sound is too large for this provider").into());
        }

        let mut endpoint = Url::parse(&entry.endpoint).map_err(|_| invalid_adapter())?;
        if let Some(voice) = &voice {
            endpoint
                .path_segments_mut()
                .map_err(|_| invalid_adapter())?
                .push(voice);
        }
        if !entry.query.is_empty() {
            endpoint.query_pairs_mut().extend_pairs(&entry.query);
        }
        let header = HeaderName::from_bytes(adapter.credential_header.as_bytes())
            .map_err(|_| invalid_adapter())?;
        if matches!(
            header.as_str(),
            "host" | "content-type" | "content-length" | "transfer-encoding" | "connection"
        ) {
            return Err(invalid_adapter().into());
        }
        let mut credential = zeroize::Zeroizing::new(adapter.credential_prefix.as_bytes().to_vec());
        credential.extend_from_slice(&secret);
        let mut value = HeaderValue::from_bytes(&credential)
            .map_err(|_| io::Error::other("Invalid credential format"))?;
        value.set_sensitive(true);
        let mut form = reqwest::blocking::multipart::Form::new();
        for (key, text) in &entry.fields {
            form = form.text(key.clone(), text.clone());
        }
        form = form.part(
            entry.file_field.clone(),
            reqwest::blocking::multipart::Part::bytes(bytes)
                .file_name("source.mp3")
                .mime_str("audio/mpeg")
                .map_err(|_| io::Error::other("Could not encode the sound"))?,
        );
        let client = speech_client_builder()
            .timeout(Duration::from_secs(900))
            .build()
            .map_err(|_| io::Error::other("Could not initialize audio transport"))?;
        let response = client
            .post(endpoint)
            .header(header, value)
            .multipart(form)
            .send()
            .map_err(|_| io::Error::other("The request to the provider failed"))?;
        let status = response.status().as_u16();
        match status {
            200..=299 => {}
            401 | 403 => {
                return Err(io::Error::other(format!(
                    "The provider refused the key for this (HTTP {status}). Check that the key is allowed to use it and that your plan includes it."
                ))
                .into());
            }
            402 => {
                return Err(io::Error::other(
                    "The provider says your plan doesn't include this or is out of credits (HTTP 402). Upgrade the plan or add credits, then retry.",
                )
                .into());
            }
            _ => {
                let reason = provider_reason(response);
                return Err(io::Error::other(format!(
                    "The provider returned HTTP {status}{}",
                    reason.map(|text| format!(": {text}")).unwrap_or_default()
                ))
                .into());
            }
        }
        let mut output = Vec::new();
        response
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .map_err(|_| io::Error::other("Could not read the provider's audio"))?;
        if output.len() as u64 > MAX_OUTPUT_BYTES {
            return Err(io::Error::other("The provider's audio is too large").into());
        }
        let extension = audio_extension(&output).ok_or_else(|| {
            io::Error::other("The provider did not return MP3, WAV, FLAC or Ogg audio")
        })?;
        let file = scratch.path().join(format!(
            "{}.{extension}",
            result_stem(&source.name, transform)
        ));
        std::fs::write(&file, &output)?;
        crate::source_transcripts::media_duration(&file)
            .map_err(|_| io::Error::other("The provider's audio could not be read"))?;

        let mut writer = WorkspaceStore::open(database)?;
        let asset = writer
            .import_asset(&crate::asset_root(database), workspace_id, &file)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let made = match transform {
            AudioTransform::VoiceChange => format!(
                "Made from {} by {plugin}'s voice changer with voice {}.",
                source.name,
                voice.as_deref().unwrap_or_default()
            ),
            AudioTransform::VoiceIsolation => {
                format!("The voice from {}, isolated by {plugin}.", source.name)
            }
        };
        writer.set_asset_notes(workspace_id, asset.id, &made)?;
        writer.set_asset_organization(
            workspace_id,
            asset.id,
            crate::AssetOrganization {
                group: None,
                labels: vec![transform.label().to_owned()],
            },
        )?;
        Ok((asset.id, plugin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_returned_audio() {
        assert_eq!(audio_extension(b"ID3\x04rest"), Some("mp3"));
        assert_eq!(audio_extension(&[0xff, 0xfb, 0x90, 0x00]), Some("mp3"));
        assert_eq!(audio_extension(b"RIFF\0\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(audio_extension(b"fLaC\0\0"), Some("flac"));
        assert_eq!(audio_extension(b"OggS\0"), Some("ogg"));
        assert_eq!(audio_extension(b"{\"detail\":1}"), None);
    }

    #[test]
    fn results_are_named_after_their_source() {
        assert_eq!(
            result_stem("Interview Take 2.mp4", AudioTransform::VoiceIsolation),
            "interview-take-2-voice-isolated"
        );
        assert_eq!(
            result_stem("???.wav", AudioTransform::VoiceChange),
            "audio-voice-changed"
        );
    }

    #[test]
    fn descriptors_need_safe_fields() {
        let entry =
            |value: serde_json::Value| serde_json::from_value::<TransformEntry>(value).unwrap();
        let base = serde_json::json!({
            "endpoint": "https://api.example.com/v1/voice-changer", "voiceSegment": true,
            "fileField": "audio", "fields": {"model_id": "sts"}, "query": {},
            "maxInputSeconds": 300, "maxInputBytes": 1000
        });
        assert!(valid_entry(&entry(base.clone())));
        let mut bad = base.clone();
        bad["endpoint"] = "http://api.example.com/v1".into();
        assert!(!valid_entry(&entry(bad)));
        let mut bad = base.clone();
        bad["fields"] = serde_json::json!({"audio": "x"});
        assert!(!valid_entry(&entry(bad)));
        let mut bad = base;
        bad["maxInputSeconds"] = 0.into();
        assert!(!valid_entry(&entry(bad)));
    }

    #[test]
    fn the_bundled_elevenlabs_descriptor_is_accepted() {
        if bundled_manifests()
            .iter()
            .any(|manifest| manifest.id == "elevenlabs")
        {
            let adapter = transform_adapter("elevenlabs").unwrap().unwrap();
            assert!(adapter.transforms.contains_key("voice-change"));
            assert!(adapter.transforms.contains_key("voice-isolation"));
        }
    }
}
