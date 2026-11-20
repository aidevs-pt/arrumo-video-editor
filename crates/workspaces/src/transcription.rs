use std::{
    collections::BTreeMap,
    io::{self, Read},
    time::Duration,
};

use reqwest::{
    blocking::{Client, Request, multipart},
    header::{HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::WorkspaceStore;

const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptionAdapter {
    schema_version: u32,
    plugin_id: String,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    max_audio_bytes: u64,
    file_field: String,
    form: BTreeMap<String, String>,
    response: ResponseMapping,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResponseMapping {
    text: String,
    language: String,
    language_item: Option<String>,
    words: Option<WordMapping>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WordMapping {
    items: String,
    text: String,
    start: String,
    end: String,
    speaker: Option<String>,
    filter: Option<WordFilter>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WordFilter {
    field: String,
    equals: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Transcript {
    pub language_code: String,
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TranscriptWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub speaker_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderTranscript {
    pub provider: String,
    pub transcript: Transcript,
}

impl WorkspaceStore {
    pub fn transcribe_wav(
        &self,
        workspace_id: i64,
        plugin_id: &str,
        audio: impl Read,
    ) -> io::Result<Transcript> {
        let adapter = bundled_transcription_adapter(plugin_id)?;
        let plugin = self
            .plugin_manifest(plugin_id)
            .map_err(|_| io::Error::other("Could not load transcription plugin"))?;
        if !plugin.capabilities.contains_key("transcription") {
            return Err(io::Error::other("Plugin does not declare transcription"));
        }
        let bytes = read_wav(audio, adapter.max_audio_bytes)?;
        let secret = self
            .provider_secret(plugin_id, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| io::Error::other("Transcription credential is not configured"))?;
        execute_transcription(&adapter, &bytes, secret.as_slice()).map_err(AttemptError::into_io)
    }

    pub fn transcribe_preferred_wav(
        &self,
        workspace_id: i64,
        audio: impl Read,
    ) -> io::Result<ProviderTranscript> {
        let order = self
            .provider_preferences("transcription", Some(workspace_id))
            .map_err(|_| io::Error::other("Could not resolve transcription providers"))?
            .effective;
        if order.is_empty() {
            return Err(io::Error::other("No transcription providers selected"));
        }
        let bytes = read_wav(audio, MAX_AUDIO_BYTES)?;
        let mut rejected = None;
        for provider in order {
            let Some(adapter) = find_bundled_transcription_adapter(&provider)? else {
                continue;
            };
            if bytes.len() as u64 > adapter.max_audio_bytes {
                continue;
            }
            let Some(secret) =
                self.provider_secret(&provider, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            match execute_transcription(&adapter, &bytes, secret.as_slice()) {
                Ok(transcript) => {
                    return Ok(ProviderTranscript {
                        provider,
                        transcript,
                    });
                }
                Err(AttemptError::Rejected(error)) => rejected = Some(error),
                Err(AttemptError::Failed(error)) => return Err(error),
            }
        }
        Err(rejected.unwrap_or_else(|| {
            io::Error::other("No selected transcription provider is configured for this WAV")
        }))
    }
}

fn read_wav(audio: impl Read, maximum: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    audio.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum
        || bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WAVE")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Expected a WAV file of at most {maximum} bytes"),
        ));
    }
    Ok(bytes)
}

enum AttemptError {
    Rejected(io::Error),
    Failed(io::Error),
}

impl AttemptError {
    fn from_status(status: reqwest::StatusCode) -> Self {
        let error = io::Error::other(format!(
            "Transcription provider returned HTTP {}",
            status.as_u16()
        ));
        // Only fall back once the provider definitively refused this request.
        match status {
            reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
            | reqwest::StatusCode::TOO_MANY_REQUESTS => Self::Rejected(error),
            _ => Self::Failed(error),
        }
    }

    fn into_io(self) -> io::Error {
        match self {
            Self::Rejected(error) | Self::Failed(error) => error,
        }
    }
}

impl From<io::Error> for AttemptError {
    fn from(error: io::Error) -> Self {
        Self::Failed(error)
    }
}

fn execute_transcription(
    adapter: &TranscriptionAdapter,
    bytes: &[u8],
    secret: &[u8],
) -> Result<Transcript, AttemptError> {
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| io::Error::other("Could not initialize transcription transport"))?;
    let request = transcription_request(&client, adapter, bytes.to_vec(), secret)?;
    let response = client
        .execute(request)
        .map_err(|_| io::Error::other("Transcription request failed"))?;
    if !response.status().is_success() {
        return Err(AttemptError::from_status(response.status()));
    }
    decode_transcript(response, &adapter.response).map_err(AttemptError::Failed)
}

fn bundled_transcription_adapter(plugin_id: &str) -> io::Result<TranscriptionAdapter> {
    find_bundled_transcription_adapter(plugin_id)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Plugin has no bundled transcription adapter",
        )
    })
}

fn find_bundled_transcription_adapter(plugin_id: &str) -> io::Result<Option<TranscriptionAdapter>> {
    for (manifest, source) in crate::plugin_catalog::bundled_manifests()
        .iter()
        .zip(crate::plugin_catalog::BUNDLED)
    {
        let Some(source) = source.transcription.filter(|_| manifest.id == plugin_id) else {
            continue;
        };
        let adapter: TranscriptionAdapter = serde_json::from_slice(source)
            .map_err(|_| io::Error::other("Invalid bundled transcription adapter"))?;
        if adapter.plugin_id != manifest.id
            || adapter.schema_version != 2
            || !manifest.capabilities.contains_key("transcription")
            || !(1..=MAX_AUDIO_BYTES).contains(&adapter.max_audio_bytes)
            || adapter.form.contains_key(&adapter.file_field)
        {
            return Err(io::Error::other(
                "Invalid transcription adapter declaration",
            ));
        }
        crate::validate_registry_url(&adapter.endpoint)
            .map_err(|_| io::Error::other("Invalid transcription endpoint"))?;
        return Ok(Some(adapter));
    }
    Ok(None)
}

fn transcription_request(
    client: &Client,
    adapter: &TranscriptionAdapter,
    audio: Vec<u8>,
    secret: &[u8],
) -> io::Result<Request> {
    let header = HeaderName::from_bytes(adapter.credential_header.as_bytes())
        .map_err(|_| io::Error::other("Invalid adapter credential header"))?;
    let mut credentials = zeroize::Zeroizing::new(Vec::new());
    credentials.extend_from_slice(adapter.credential_prefix.as_bytes());
    credentials.extend_from_slice(secret);
    let mut credential = HeaderValue::from_bytes(&credentials)
        .map_err(|_| io::Error::other("Invalid transcription credential format"))?;
    credential.set_sensitive(true);
    let audio = multipart::Part::bytes(audio)
        .file_name("source.wav")
        .mime_str("audio/wav")
        .map_err(|_| io::Error::other("Could not build transcription upload"))?;
    let mut form = multipart::Form::new().part(adapter.file_field.clone(), audio);
    for (field, value) in &adapter.form {
        form = form.text(field.clone(), value.clone());
    }
    client
        .post(&adapter.endpoint)
        .header(header, credential)
        .multipart(form)
        .build()
        .map_err(|_| io::Error::other("Could not build transcription request"))
}

fn decode_transcript(response: impl Read, mapping: &ResponseMapping) -> io::Result<Transcript> {
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io::Error::other("Could not read transcription response"))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(io::Error::other("Transcription response exceeds 16 MiB"));
    }
    let document: Value = serde_json::from_slice(&bytes).map_err(|_| invalid_transcript())?;
    let text = mapped_text(&document, &mapping.text)?.to_owned();
    let language_code = if let Some(item) = &mapping.language_item {
        match document.pointer(&mapping.language) {
            None => "und".into(),
            Some(Value::Array(items)) => match items.as_slice() {
                [] => "und".into(),
                [language] => mapped_text(language, item)?.to_owned(),
                _ => "mul".into(),
            },
            _ => return Err(invalid_transcript()),
        }
    } else {
        mapped_text(&document, &mapping.language)?.to_owned()
    };
    let words = if let Some(words) = &mapping.words {
        let items = document
            .pointer(&words.items)
            .and_then(Value::as_array)
            .ok_or_else(invalid_transcript)?;
        items
            .iter()
            .filter(|item| {
                words.filter.as_ref().is_none_or(|filter| {
                    item.pointer(&filter.field).and_then(Value::as_str)
                        == Some(filter.equals.as_str())
                })
            })
            .map(|item| {
                let start = item.pointer(&words.start).and_then(Value::as_f64);
                let end = item.pointer(&words.end).and_then(Value::as_f64);
                match (start, end) {
                    (Some(start), Some(end)) if start >= 0.0 && end >= start && end.is_finite() => {
                        Ok(TranscriptWord {
                            text: mapped_text(item, &words.text)?.to_owned(),
                            start,
                            end,
                            speaker_id: words
                                .speaker
                                .as_ref()
                                .and_then(|pointer| item.pointer(pointer))
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        })
                    }
                    _ => Err(io::Error::other(
                        "Provider returned invalid word timestamps",
                    )),
                }
            })
            .collect::<io::Result<_>>()?
    } else {
        Vec::new()
    };
    Ok(Transcript {
        language_code,
        text,
        words,
    })
}

fn mapped_text<'a>(value: &'a Value, pointer: &str) -> io::Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(invalid_transcript)
}

fn invalid_transcript() -> io::Error {
    io::Error::other("Provider returned an invalid transcript")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TranscriptionAdapter {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 2, "pluginId": "fixture",
            "endpoint": "https://example.org/transcribe",
            "credentialSetting": "api-key", "credentialHeader": "x-secret",
            "credentialPrefix": "Token ", "maxAudioBytes": 1024,
            "fileField": "source", "form": {"preset": "accurate"},
            "response": {
                "text": "/result", "language": "/languages", "languageItem": "/id",
                "words": {"items": "/tokens", "text": "/value", "start": "/from",
                    "end": "/to", "speaker": "/voice", "filter": {"field": "/kind", "equals": "word"}}
            }
        })).unwrap()
    }

    #[test]
    fn builds_requests_from_descriptor_without_exposing_credentials() {
        let adapter = fixture();
        let client = Client::builder().https_only(true).build().unwrap();
        let request = transcription_request(&client, &adapter, vec![0], b"fixture-secret").unwrap();
        assert_eq!(request.url().as_str(), adapter.endpoint);
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.headers()["x-secret"], "Token fixture-secret");
        assert!(request.headers()["x-secret"].is_sensitive());
        assert!(!format!("{request:?}").contains("fixture-secret"));
    }

    #[test]
    fn maps_transcript_fields_and_rejects_invalid_timing() {
        let adapter = fixture();
        let payload = br#"{"result":"Hello","languages":[{"id":"en"}],"tokens":[
            {"kind":"word","value":"Hello","from":0.1,"to":0.3,"voice":"one"},
            {"kind":"spacing"}
        ]}"#;
        let transcript = decode_transcript(payload.as_slice(), &adapter.response).unwrap();
        assert_eq!(transcript.language_code, "en");
        assert_eq!(transcript.words.len(), 1);
        assert_eq!(transcript.words[0].speaker_id.as_deref(), Some("one"));
        let bad = br#"{"result":"Hello","languages":[],"tokens":[
            {"kind":"word","value":"Hello","from":0.3,"to":0.1}
        ]}"#;
        assert!(decode_transcript(bad.as_slice(), &adapter.response).is_err());
    }

    #[test]
    fn absent_timing_mapping_does_not_invent_timings() {
        let mut mapping = fixture().response;
        mapping.words = None;
        let transcript = decode_transcript(br#"{"result":"Hello"}"#.as_slice(), &mapping).unwrap();
        assert_eq!(transcript.language_code, "und");
        assert!(transcript.words.is_empty());
    }

    #[test]
    fn only_explicit_auth_and_rate_rejection_can_fall_back_after_dispatch() {
        use reqwest::StatusCode;
        assert!(matches!(
            AttemptError::from_status(StatusCode::TOO_MANY_REQUESTS),
            AttemptError::Rejected(_)
        ));
        assert!(matches!(
            AttemptError::from_status(StatusCode::GATEWAY_TIMEOUT),
            AttemptError::Failed(_)
        ));
    }
}
