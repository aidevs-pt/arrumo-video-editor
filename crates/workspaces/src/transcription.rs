use std::{
    io::{self, Read},
    time::Duration,
};

use reqwest::{
    blocking::{Client, Request, multipart},
    header::{HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const BUNDLED_TRANSCRIPTION: &[&str] = &[
    include_str!("../plugins/elevenlabs/transcription.json"),
    include_str!("../plugins/openai/transcription.json"),
];

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Protocol {
    TimedWordsMultipart,
    TextMultipart,
}

impl Protocol {
    fn max_audio_bytes(self) -> u64 {
        match self {
            Self::TimedWordsMultipart => MAX_AUDIO_BYTES,
            Self::TextMultipart => 25_000_000,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptionAdapter {
    schema_version: u32,
    plugin_id: String,
    protocol: Protocol,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    model: String,
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

#[derive(Deserialize)]
struct TextTranscript {
    text: String,
    #[serde(default)]
    languages: Vec<DetectedLanguage>,
}

#[derive(Deserialize)]
struct DetectedLanguage {
    code: String,
}

#[derive(Deserialize)]
struct WireTranscript {
    language_code: String,
    text: String,
    words: Vec<WireToken>,
}

#[derive(Deserialize)]
struct WireToken {
    #[serde(rename = "type")]
    kind: String,
    text: String,
    start: Option<f64>,
    end: Option<f64>,
    speaker_id: Option<String>,
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
        let bytes = read_wav(audio, adapter.protocol.max_audio_bytes())?;
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
            if bytes.len() as u64 > adapter.protocol.max_audio_bytes() {
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
    decode_transcript(response, adapter.protocol).map_err(AttemptError::Failed)
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
    for source in BUNDLED_TRANSCRIPTION {
        let adapter: TranscriptionAdapter = serde_json::from_str(source)
            .map_err(|_| io::Error::other("Invalid bundled transcription adapter"))?;
        if adapter.plugin_id == plugin_id {
            if adapter.schema_version != 1 {
                return Err(io::Error::other(
                    "Unsupported transcription adapter protocol",
                ));
            }
            crate::validate_registry_url(&adapter.endpoint)
                .map_err(|_| io::Error::other("Invalid transcription endpoint"))?;
            return Ok(Some(adapter));
        }
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
    if matches!(adapter.protocol, Protocol::TextMultipart) {
        credentials.extend_from_slice(b"Bearer ");
    }
    credentials.extend_from_slice(secret);
    let mut credential = HeaderValue::from_bytes(&credentials)
        .map_err(|_| io::Error::other("Invalid transcription credential format"))?;
    credential.set_sensitive(true);
    let audio = multipart::Part::bytes(audio)
        .file_name("source.wav")
        .mime_str("audio/wav")
        .map_err(|_| io::Error::other("Could not build transcription upload"))?;
    let form = multipart::Form::new().part("file", audio);
    let form = match adapter.protocol {
        Protocol::TimedWordsMultipart => form
            .text("model_id", adapter.model.clone())
            .text("timestamps_granularity", "word"),
        Protocol::TextMultipart => form.text("model", adapter.model.clone()),
    };
    client
        .post(&adapter.endpoint)
        .header(header, credential)
        .multipart(form)
        .build()
        .map_err(|_| io::Error::other("Could not build transcription request"))
}

fn decode_transcript(response: impl Read, protocol: Protocol) -> io::Result<Transcript> {
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io::Error::other("Could not read transcription response"))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(io::Error::other("Transcription response exceeds 16 MiB"));
    }
    if matches!(protocol, Protocol::TextMultipart) {
        let result: TextTranscript = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::other("Provider returned an invalid transcript"))?;
        let language_code = match result.languages.as_slice() {
            [language] => language.code.clone(),
            [] => "und".into(),
            _ => "mul".into(),
        };
        return Ok(Transcript {
            language_code,
            text: result.text,
            words: Vec::new(),
        });
    }
    let decoded: WireTranscript = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::other("Provider returned an invalid transcript"))?;
    let words = decoded
        .words
        .into_iter()
        .filter(|token| token.kind == "word")
        .map(|token| match (token.start, token.end) {
            (Some(start), Some(end))
                if start.is_finite() && end.is_finite() && start >= 0.0 && end >= start =>
            {
                Ok(TranscriptWord {
                    text: token.text,
                    start,
                    end,
                    speaker_id: token.speaker_id,
                })
            }
            _ => Err(io::Error::other(
                "Provider returned invalid word timestamps",
            )),
        })
        .collect::<io::Result<_>>()?;
    Ok(Transcript {
        language_code: decoded.language_code,
        text: decoded.text,
        words,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_adapter_builds_authenticated_request_without_exposing_header_in_debug() {
        let adapter = bundled_transcription_adapter("elevenlabs").unwrap();
        let client = Client::builder().https_only(true).build().unwrap();
        let request = transcription_request(&client, &adapter, vec![0], b"fixture-secret").unwrap();
        assert_eq!(request.url().as_str(), adapter.endpoint);
        assert_eq!(request.method(), reqwest::Method::POST);
        assert!(request.headers()[adapter.credential_header.as_str()].is_sensitive());
        assert!(!format!("{request:?}").contains("fixture-secret"));
        let openai = bundled_transcription_adapter("openai").unwrap();
        let request = transcription_request(&client, &openai, vec![0], b"fixture-secret").unwrap();
        assert_eq!(request.headers()["authorization"], "Bearer fixture-secret");
        assert!(request.headers()["authorization"].is_sensitive());
        assert!(!format!("{request:?}").contains("fixture-secret"));
    }

    #[test]
    fn normalizes_provider_words_and_rejects_invalid_timing() {
        let payload = br#"{"language_code":"eng","text":"Hello world","words":[
            {"type":"word","text":"Hello","start":0.1,"end":0.3,"speaker_id":"speaker_1"},
            {"type":"spacing","text":" "},
            {"type":"word","text":"world","start":0.4,"end":0.7}
        ]}"#;
        let transcript =
            decode_transcript(payload.as_slice(), Protocol::TimedWordsMultipart).unwrap();
        assert_eq!(transcript.words.len(), 2);
        assert_eq!(transcript.words[0].speaker_id.as_deref(), Some("speaker_1"));
        let bad = br#"{"language_code":"en","text":"a","words":[{"type":"word","text":"a","start":1,"end":0}]}"#;
        assert!(decode_transcript(bad.as_slice(), Protocol::TimedWordsMultipart).is_err());
    }
    #[test]
    fn normalizes_openai_text_without_inventing_word_timings() {
        let payload = br#"{"text":"Bonjour","languages":[{"code":"fr"}]}"#;
        let transcript = decode_transcript(payload.as_slice(), Protocol::TextMultipart).unwrap();
        assert_eq!(transcript.language_code, "fr");
        assert_eq!(transcript.text, "Bonjour");
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
