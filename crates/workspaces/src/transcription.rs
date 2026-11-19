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
const BUNDLED_TRANSCRIPTION: &[&str] = &[include_str!("../plugins/elevenlabs/transcription.json")];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptionAdapter {
    schema_version: u32,
    plugin_id: String,
    protocol: String,
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
        let mut bytes = Vec::new();
        audio.take(MAX_AUDIO_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_AUDIO_BYTES
            || bytes.get(..4) != Some(b"RIFF")
            || bytes.get(8..12) != Some(b"WAVE")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected a WAV file of at most 32 MiB",
            ));
        }
        let secret = self
            .provider_secret(plugin_id, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| io::Error::other("Transcription credential is not configured"))?;
        let client = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| io::Error::other("Could not initialize transcription transport"))?;
        let request = transcription_request(&client, &adapter, bytes, secret.as_slice())?;
        let response = client
            .execute(request)
            .map_err(|_| io::Error::other("Transcription request failed"))?;
        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "Transcription provider returned HTTP {}",
                response.status().as_u16()
            )));
        }
        decode_transcript(response)
    }
}

fn bundled_transcription_adapter(plugin_id: &str) -> io::Result<TranscriptionAdapter> {
    for source in BUNDLED_TRANSCRIPTION {
        let adapter: TranscriptionAdapter = serde_json::from_str(source)
            .map_err(|_| io::Error::other("Invalid bundled transcription adapter"))?;
        if adapter.plugin_id == plugin_id {
            if adapter.schema_version != 1 || adapter.protocol != "timed-words-multipart" {
                return Err(io::Error::other(
                    "Unsupported transcription adapter protocol",
                ));
            }
            crate::validate_registry_url(&adapter.endpoint)
                .map_err(|_| io::Error::other("Invalid transcription endpoint"))?;
            return Ok(adapter);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Plugin has no bundled transcription adapter",
    ))
}

fn transcription_request(
    client: &Client,
    adapter: &TranscriptionAdapter,
    audio: Vec<u8>,
    secret: &[u8],
) -> io::Result<Request> {
    let header = HeaderName::from_bytes(adapter.credential_header.as_bytes())
        .map_err(|_| io::Error::other("Invalid adapter credential header"))?;
    let mut credential = HeaderValue::from_bytes(secret)
        .map_err(|_| io::Error::other("Invalid transcription credential format"))?;
    credential.set_sensitive(true);
    let audio = multipart::Part::bytes(audio)
        .file_name("source.wav")
        .mime_str("audio/wav")
        .map_err(|_| io::Error::other("Could not build transcription upload"))?;
    let form = multipart::Form::new()
        .text("model_id", adapter.model.clone())
        .text("timestamps_granularity", "word")
        .part("file", audio);
    client
        .post(&adapter.endpoint)
        .header(header, credential)
        .multipart(form)
        .build()
        .map_err(|_| io::Error::other("Could not build transcription request"))
}

fn decode_transcript(response: impl Read) -> io::Result<Transcript> {
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io::Error::other("Could not read transcription response"))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(io::Error::other("Transcription response exceeds 16 MiB"));
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
        assert!(bundled_transcription_adapter("openai").is_err());
    }

    #[test]
    fn normalizes_provider_words_and_rejects_invalid_timing() {
        let payload = br#"{"language_code":"eng","text":"Hello world","words":[
            {"type":"word","text":"Hello","start":0.1,"end":0.3,"speaker_id":"speaker_1"},
            {"type":"spacing","text":" "},
            {"type":"word","text":"world","start":0.4,"end":0.7}
        ]}"#;
        let transcript = decode_transcript(payload.as_slice()).unwrap();
        assert_eq!(transcript.words.len(), 2);
        assert_eq!(transcript.words[0].speaker_id.as_deref(), Some("speaker_1"));
        let bad = br#"{"language_code":"en","text":"a","words":[{"type":"word","text":"a","start":1,"end":0}]}"#;
        assert!(decode_transcript(bad.as_slice()).is_err());
    }
}
