//! Music and sound effects generated from a written prompt by a trusted provider plugin's
//! `audio-generation.json` descriptor, saved as workspace audio assets.

use std::{collections::BTreeMap, io, path::Path};

use reqwest::{
    Url,
    blocking::{Client, Request},
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;
use tempfile::NamedTempFile;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
    speech_synthesis::{AudioFormat, execute_speech, speech_client_builder, valid_key},
};

/// What a creator asks a provider to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioKind {
    Music,
    SoundEffect,
}

impl AudioKind {
    fn capability(self) -> &'static str {
        match self {
            Self::Music => "music-generation",
            Self::SoundEffect => "sound-effect-generation",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Music => "Music",
            Self::SoundEffect => "Sound effect",
        }
    }

    fn stem(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::SoundEffect => "sound",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioAdapter {
    schema_version: u32,
    plugin_id: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    response_format: AudioFormat,
    kinds: BTreeMap<String, KindAdapter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KindAdapter {
    endpoint: String,
    prompt_field: String,
    duration_field: String,
    duration_unit: DurationUnit,
    min_seconds: f64,
    max_seconds: f64,
    max_prompt_characters: usize,
    body: BTreeMap<String, Value>,
    query: BTreeMap<String, String>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DurationUnit {
    Seconds,
    Milliseconds,
}

fn invalid_adapter() -> io::Error {
    io::Error::other("Invalid audio generation adapter")
}

fn audio_adapter(plugin_id: &str, kind: AudioKind) -> io::Result<(AudioAdapter, String)> {
    let (manifest, source) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
        .ok_or_else(|| io::Error::other("Audio plugin is unavailable"))?;
    let bytes = source
        .audio_generation
        .ok_or_else(|| io::Error::other("Plugin has no trusted audio adapter"))?;
    let adapter: AudioAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    let key = match kind {
        AudioKind::Music => "music",
        AudioKind::SoundEffect => "sound-effect",
    };
    let has_secret = manifest.settings.iter().flatten().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    let entry = adapter
        .kinds
        .get(key)
        .ok_or_else(|| io::Error::other("That provider does not make this kind of audio"))?;
    if adapter.schema_version != 1
        || adapter.plugin_id != manifest.id
        || !has_secret
        || !manifest.capabilities.contains_key(kind.capability())
        || !matches!(adapter.credential_prefix.as_str(), "" | "Bearer ")
        || !valid_kind(entry)
    {
        return Err(invalid_adapter());
    }
    Ok((adapter, key.to_owned()))
}

fn valid_kind(entry: &KindAdapter) -> bool {
    let scalar = |value: &Value| match value {
        Value::String(text) => text.chars().count() <= 256,
        Value::Bool(_) => true,
        Value::Number(number) => number.as_f64().is_some_and(f64::is_finite),
        _ => false,
    };
    valid_endpoint(&entry.endpoint)
        && valid_key(&entry.prompt_field)
        && valid_key(&entry.duration_field)
        && entry.prompt_field != entry.duration_field
        && !entry.body.contains_key(&entry.prompt_field)
        && !entry.body.contains_key(&entry.duration_field)
        && entry.min_seconds > 0.0
        && entry.min_seconds <= entry.max_seconds
        && entry.max_seconds <= 3600.0
        && (1..=10_000).contains(&entry.max_prompt_characters)
        && entry.body.len() <= 16
        && entry.query.len() <= 16
        && entry
            .body
            .iter()
            .all(|(key, value)| valid_key(key) && scalar(value))
        && entry
            .query
            .iter()
            .all(|(key, value)| valid_key(key) && value.chars().count() <= 256)
}

fn valid_endpoint(endpoint: &str) -> bool {
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

/// The JSON body for one request: the fixed fields, the prompt and the duration in the
/// provider's unit.
fn audio_body(entry: &KindAdapter, prompt: &str, seconds: f64) -> io::Result<Value> {
    let prompt = prompt.trim();
    if prompt.is_empty()
        || prompt.contains('\0')
        || prompt.chars().count() > entry.max_prompt_characters
    {
        return Err(io::Error::other(format!(
            "Describe the sound in up to {} characters",
            entry.max_prompt_characters
        )));
    }
    if !seconds.is_finite() || seconds < entry.min_seconds || seconds > entry.max_seconds {
        return Err(io::Error::other(format!(
            "Choose a length from {} to {} seconds",
            entry.min_seconds, entry.max_seconds
        )));
    }
    let mut body: serde_json::Map<String, Value> = entry
        .body
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    body.insert(entry.prompt_field.clone(), Value::from(prompt));
    body.insert(
        entry.duration_field.clone(),
        match entry.duration_unit {
            DurationUnit::Seconds => Value::from((seconds * 10.0).round() / 10.0),
            DurationUnit::Milliseconds => Value::from((seconds * 1000.0).round() as u64),
        },
    );
    Ok(Value::Object(body))
}

fn audio_request(
    client: &Client,
    adapter: &AudioAdapter,
    entry: &KindAdapter,
    body: &Value,
    secret: &[u8],
) -> io::Result<Request> {
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
        .map_err(|_| io::Error::other("Invalid audio credential format"))?;
    value.set_sensitive(true);
    let mut endpoint = Url::parse(&entry.endpoint).map_err(|_| invalid_adapter())?;
    if !entry.query.is_empty() {
        endpoint.query_pairs_mut().extend_pairs(&entry.query);
    }
    client
        .post(endpoint)
        .header(header, value)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_vec(body).map_err(|_| io::Error::other("Could not encode prompt"))?)
        .build()
        .map_err(|_| io::Error::other("Could not build audio request"))
}

impl WorkspaceStore {
    /// Generates audio like `generate_library_audio_untracked` and counts it in usage data.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_library_audio(
        &self,
        database: &Path,
        workspace_id: i64,
        kind: AudioKind,
        prompt: &str,
        seconds: f64,
        notes: &str,
        provider: Option<&str>,
        label: Option<&str>,
    ) -> Result<(i64, String), Box<dyn std::error::Error>> {
        let generated = self.generate_library_audio_untracked(
            database,
            workspace_id,
            kind,
            prompt,
            seconds,
            notes,
            provider,
            label,
        )?;
        self.track_usage(
            "media_generated",
            &[
                ("kind", serde_json::json!("audio")),
                ("provider", serde_json::json!(generated.1)),
            ],
        );
        Ok(generated)
    }

    /// Makes music or a sound effect from `prompt` with the workspace's first provider for
    /// that kind (or `provider`) and adds it to the workspace assets with `notes`. Returns the
    /// asset ID and the provider used. The asset is labelled `label`, or Music / Sound effect.
    #[allow(clippy::too_many_arguments)]
    fn generate_library_audio_untracked(
        &self,
        database: &Path,
        workspace_id: i64,
        kind: AudioKind,
        prompt: &str,
        seconds: f64,
        notes: &str,
        provider: Option<&str>,
        label: Option<&str>,
    ) -> Result<(i64, String), Box<dyn std::error::Error>> {
        if notes.len() > 8000 {
            return Err(io::Error::other("Notes are too long").into());
        }
        let capability = kind.capability();
        if let Some(id) = provider {
            self.check_named_provider(id)?;
        }
        let offered = self
            .capability_index(Some(workspace_id))?
            .get(capability)
            .map(|providers| {
                providers
                    .iter()
                    .map(|entry| entry.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let plugin = match provider {
            Some(id) if offered.iter().any(|entry| entry == id) => id.to_owned(),
            Some(_) => {
                return Err(
                    io::Error::other("That provider does not make this kind of audio").into(),
                );
            }
            None => self
                .capture_provider_orders(workspace_id, &BTreeMap::new())?
                .get(capability)
                .and_then(|order| order.effective.first().cloned())
                .or_else(|| offered.first().cloned())
                .ok_or_else(|| {
                    io::Error::other(
                        "Turn on a music or sound provider in Settings › Services and keys",
                    )
                })?,
        };
        let (adapter, key) = audio_adapter(&plugin, kind)?;
        let entry = &adapter.kinds[&key];
        let body = audio_body(entry, prompt, seconds)?;
        let secret = self
            .provider_secret(&plugin, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| {
                io::Error::other("Add the provider's key in Settings › Services and keys")
            })?;

        let scratch = tempfile::tempdir()?;
        let words = prompt
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .take(6)
            .collect::<Vec<_>>()
            .join("-")
            .to_ascii_lowercase();
        let stem = if words.is_empty() {
            kind.stem().to_owned()
        } else {
            format!("{}-{words}", kind.stem())
        };
        let file = scratch
            .path()
            .join(format!("{stem}.{}", adapter.response_format.extension()));
        let client = speech_client_builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|_| io::Error::other("Could not initialize audio transport"))?;
        let request = audio_request(&client, &adapter, entry, &body, &secret)?;
        let pending = NamedTempFile::new_in(scratch.path())?;
        execute_speech(&client, request, &adapter.response_format, pending, &file)?;

        let mut writer = WorkspaceStore::open(database)?;
        let asset = writer
            .import_asset(&crate::asset_root(database), workspace_id, &file)
            .map_err(|error| io::Error::other(error.to_string()))?;
        if !notes.trim().is_empty() {
            writer.set_asset_notes(workspace_id, asset.id, notes.trim())?;
        }
        let label = label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(kind.label());
        writer.set_asset_organization(
            workspace_id,
            asset.id,
            crate::AssetOrganization {
                group: None,
                labels: vec![label.to_owned()],
            },
        )?;
        Ok((asset.id, plugin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn music() -> KindAdapter {
        serde_json::from_value(serde_json::json!({
            "endpoint": "https://api.example.com/v1/music", "promptField": "prompt",
            "durationField": "music_length_ms", "durationUnit": "milliseconds",
            "minSeconds": 3, "maxSeconds": 600, "maxPromptCharacters": 20,
            "body": {"model_id": "music_v1", "force_instrumental": true},
            "query": {"output_format": "mp3_44100_128"}
        }))
        .unwrap()
    }

    #[test]
    fn body_carries_the_prompt_and_the_length_in_the_providers_unit() {
        let entry = music();
        assert!(valid_kind(&entry));
        assert_eq!(
            audio_body(&entry, "  calm piano ", 30.25).unwrap(),
            serde_json::json!({"model_id": "music_v1", "force_instrumental": true,
                "prompt": "calm piano", "music_length_ms": 30250})
        );
        let mut seconds = music();
        seconds.duration_unit = DurationUnit::Seconds;
        seconds.min_seconds = 0.5;
        assert_eq!(
            audio_body(&seconds, "pop", 1.26).unwrap()["music_length_ms"],
            1.3
        );
        for (prompt, length) in [
            ("", 30.0),
            ("a very long description indeed", 30.0),
            ("ok", 2.0),
            ("ok", f64::NAN),
        ] {
            assert!(
                audio_body(&entry, prompt, length).is_err(),
                "{prompt} {length}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_descriptors() {
        let mut entry = music();
        entry.endpoint = "http://api.example.com/v1/music".into();
        assert!(!valid_kind(&entry));
        let mut entry = music();
        entry.body.insert("prompt".into(), Value::from("x"));
        assert!(!valid_kind(&entry));
        let mut entry = music();
        entry
            .body
            .insert("nested".into(), serde_json::json!({"a": 1}));
        assert!(!valid_kind(&entry));
        let mut entry = music();
        entry.min_seconds = 700.0;
        assert!(!valid_kind(&entry));
    }

    #[test]
    fn the_bundled_elevenlabs_descriptor_is_accepted() {
        if bundled_manifests()
            .iter()
            .any(|manifest| manifest.id == "elevenlabs")
        {
            assert!(audio_adapter("elevenlabs", AudioKind::Music).is_ok());
            assert!(audio_adapter("elevenlabs", AudioKind::SoundEffect).is_ok());
        }
    }
}
