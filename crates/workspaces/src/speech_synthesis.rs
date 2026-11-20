use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    path::Path,
    time::Duration,
};

use reqwest::{
    Url,
    blocking::{Client, Request},
    header::{HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
};

const MAX_AUDIO_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpeechAdapter {
    schema_version: u32,
    plugin_id: String,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    text_field: String,
    body: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
    response_format: AudioFormat,
    max_text_characters: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AudioFormat {
    Mp3,
    Wav,
}

impl AudioFormat {
    fn extension(&self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
        }
    }

    fn accepts(&self, media_type: &str) -> bool {
        match self {
            Self::Mp3 => media_type.eq_ignore_ascii_case("audio/mpeg"),
            Self::Wav => ["audio/wav", "audio/wave", "audio/x-wav"]
                .iter()
                .any(|expected| media_type.eq_ignore_ascii_case(expected)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechProviderStatus {
    response_format: &'static str,
    max_text_characters: usize,
    credential_source: Option<PluginSettingScope>,
    adapter_sha256: String,
}

impl WorkspaceStore {
    pub fn speech_provider_status(
        &self,
        workspace_id: i64,
        plugin_id: &str,
    ) -> io::Result<SpeechProviderStatus> {
        let (adapter, descriptor) = speech_adapter(plugin_id)?;
        speech_endpoint(&adapter, "voice", "a")?;
        let status = self.plugin_credential_status(
            plugin_id,
            &adapter.credential_setting,
            Some(workspace_id),
        )?;
        Ok(SpeechProviderStatus {
            response_format: adapter.response_format.extension(),
            max_text_characters: adapter.max_text_characters,
            credential_source: status.source,
            adapter_sha256: crate::plugin_package::package_sha256(descriptor)
                .map_err(|_| io::Error::other("Could not hash speech descriptor"))?,
        })
    }

    pub fn synthesize_speech_to_file(
        &self,
        workspace_id: i64,
        plugin_id: &str,
        voice: &str,
        text: &str,
        output: &Path,
    ) -> io::Result<()> {
        let (adapter, _) = speech_adapter(plugin_id)?;
        let extension = adapter.response_format.extension();
        if output.extension().and_then(|value| value.to_str()) != Some(extension) {
            return Err(io::Error::other(
                "Output extension differs from the adapter audio format",
            ));
        }
        if output.symlink_metadata().is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Speech output already exists",
            ));
        }
        let directory = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let pending = NamedTempFile::new_in(directory)?;
        let client = speech_client_builder()
            .build()
            .map_err(|_| io::Error::other("Could not initialize speech transport"))?;
        let endpoint = speech_endpoint(&adapter, voice, text)?;
        let secret = self
            .provider_secret(plugin_id, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| io::Error::other("Speech credential is not configured"))?;
        let request = speech_request(&client, &adapter, endpoint, text, &secret)?;
        execute_speech(&client, request, &adapter.response_format, pending, output)
    }
}

fn speech_client_builder() -> reqwest::blocking::ClientBuilder {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
}

fn execute_speech(
    client: &Client,
    request: Request,
    format: &AudioFormat,
    mut pending: NamedTempFile,
    output: &Path,
) -> io::Result<()> {
    let response = client
        .execute(request)
        .map_err(|_| io::Error::other("Speech request failed"))?;
    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "Speech provider returned HTTP {}",
            response.status().as_u16()
        )));
    }
    let media_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.split(';').next())
        .unwrap_or_default();
    if !format.accepts(media_type.trim()) {
        return Err(io::Error::other(
            "Speech response has an unexpected media type",
        ));
    }
    let length = io::copy(
        &mut response.take(MAX_AUDIO_BYTES + 1),
        pending.as_file_mut(),
    )
    .map_err(|_| io::Error::other("Could not read or save speech response"))?;
    if length == 0 || length > MAX_AUDIO_BYTES {
        return Err(io::Error::other("Speech response size is invalid"));
    }
    pending.as_file_mut().flush()?;
    crate::motion_artifacts::validate_motion_file(
        pending.path(),
        &format!("speech.{}", format.extension()),
        "audio",
    )
    .map_err(|_| io::Error::other("Speech response could not be verified as audio"))?;
    pending.as_file().sync_all()?;
    pending
        .persist_noclobber(output)
        .map_err(|error| error.error)?;
    Ok(())
}

fn speech_adapter(plugin_id: &str) -> io::Result<(SpeechAdapter, &'static [u8])> {
    let (manifest, source) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
        .ok_or_else(|| io::Error::other("Speech plugin is unavailable"))?;
    let bytes = source
        .speech_synthesis
        .ok_or_else(|| io::Error::other("Plugin has no trusted speech adapter"))?;
    let adapter: SpeechAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    let has_secret = manifest.settings.iter().flatten().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    if adapter.schema_version != 1
        || adapter.plugin_id != manifest.id
        || !manifest.capabilities.contains_key("text-to-speech")
        || !has_secret
        || !(1..=10_000).contains(&adapter.max_text_characters)
        || !matches!(adapter.credential_prefix.as_str(), "" | "Bearer ")
        || !valid_key(&adapter.text_field)
        || adapter.body.contains_key(&adapter.text_field)
        || [&adapter.body, &adapter.query].into_iter().any(|map| {
            map.len() > 16
                || map
                    .iter()
                    .any(|(key, value)| !valid_key(key) || value.chars().count() > 256)
        })
    {
        return Err(invalid_adapter());
    }
    Ok((adapter, bytes))
}

fn valid_key(key: &str) -> bool {
    key.len() <= 64
        && key.starts_with(|c: char| c.is_ascii_alphabetic())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn speech_endpoint(adapter: &SpeechAdapter, voice: &str, text: &str) -> io::Result<Url> {
    if voice.is_empty()
        || voice.len() > 256
        || matches!(voice, "." | "..")
        || voice.chars().any(char::is_control)
        || text.trim().is_empty()
        || text.contains('\0')
        || text.chars().count() > adapter.max_text_characters
    {
        return Err(io::Error::other("Invalid voice identifier or speech text"));
    }
    let raw = adapter
        .endpoint
        .strip_prefix("https://")
        .ok_or_else(invalid_adapter)?;
    let (host, path) = raw.split_once('/').ok_or_else(invalid_adapter)?;
    if adapter.endpoint.len() > 2048
        || host.is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || path.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._~-".contains(&byte))
        })
    {
        return Err(invalid_adapter());
    }
    let mut endpoint = Url::parse(&adapter.endpoint).map_err(|_| invalid_adapter())?;
    endpoint
        .path_segments_mut()
        .map_err(|_| invalid_adapter())?
        .push(voice);
    Ok(endpoint)
}

fn speech_request(
    client: &Client,
    adapter: &SpeechAdapter,
    mut endpoint: Url,
    text: &str,
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
        .map_err(|_| io::Error::other("Invalid speech credential format"))?;
    value.set_sensitive(true);
    let mut body = adapter.body.clone();
    body.insert(adapter.text_field.clone(), text.into());
    if !adapter.query.is_empty() {
        endpoint.query_pairs_mut().extend_pairs(&adapter.query);
    }
    client
        .post(endpoint)
        .header(header, value)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::to_vec(&body)
                .map_err(|_| io::Error::other("Could not encode speech text"))?,
        )
        .build()
        .map_err(|_| io::Error::other("Could not build speech request"))
}

fn invalid_adapter() -> io::Error {
    io::Error::other("Invalid speech synthesis adapter")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SpeechAdapter {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1, "pluginId": "fixture", "endpoint": "https://speech.example/v1/speech",
            "credentialSetting": "api-key", "credentialHeader": "x-fixture-key", "credentialPrefix": "",
            "textField": "text", "body": {"model": "demo"},
            "query": {"format": "voice & music"}, "responseFormat": "wav", "maxTextCharacters": 5
        })).unwrap()
    }

    #[test]
    fn request_keeps_voice_in_one_segment_and_marks_secret_sensitive() {
        let adapter = fixture();
        let input = "olá";
        let endpoint = speech_endpoint(&adapter, "x/../?a#b", input).unwrap();
        let request =
            speech_request(&Client::new(), &adapter, endpoint, input, b"fixture-secret").unwrap();
        assert_eq!(request.url().path(), "/v1/speech/x%2F..%2F%3Fa%23b");
        assert_eq!(
            request.url().query_pairs().next().unwrap().1,
            "voice & music"
        );
        assert!(request.headers()["x-fixture-key"].is_sensitive());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                request.body().unwrap().as_bytes().unwrap()
            )
            .unwrap(),
            serde_json::json!({"text": "olá", "model": "demo"})
        );
    }

    #[test]
    fn rejects_unsafe_endpoints_and_out_of_bounds_inputs() {
        for endpoint in [
            "http://speech.example/v1",
            "https://secret@speech.example/v1",
            "https://speech.example/v1?key=value",
            "https://speech.example/v1/../speech",
            "https://speech.example/v1%2Fspeech",
            "https://speech.example/v1#fragment",
        ] {
            let mut adapter = fixture();
            adapter.endpoint = endpoint.into();
            assert!(
                speech_endpoint(&adapter, "voice", "hello").is_err(),
                "{endpoint}"
            );
        }
        assert!(speech_endpoint(&fixture(), "..", "hello").is_err());
        assert!(speech_endpoint(&fixture(), "voice", "ééééé").is_ok());
        assert!(speech_endpoint(&fixture(), "voice", "éééééé").is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn https_transport_saves_verified_audio_and_discards_error_responses() {
        use rustls::{ServerConfig, ServerConnection, StreamOwned, pki_types::PrivatePkcs8KeyDer};
        use std::{
            io::{BufRead, BufReader},
            net::TcpListener,
            sync::Arc,
            thread,
            time::Instant,
        };

        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let der = certificate.cert.der();
        let config = Arc::new(
            ServerConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![der.clone()],
                    PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()).into(),
                )
                .unwrap(),
        );
        let client = speech_client_builder()
            .no_proxy()
            .tls_certs_merge([reqwest::Certificate::from_der(der).unwrap()])
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        // 80 samples of silent 8 kHz mono PCM.
        let mut wav = b"RIFF\xc4\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\xa0\0\0\0".to_vec();
        wav.extend([0; 160]);
        let directory = tempfile::tempdir().unwrap();
        for (index, (status, extra, body, expected_error)) in [
            ("200 OK", "Content-Type: audio/wav", wav.as_slice(), None),
            (
                "200 OK",
                "Content-Type: application/json",
                b"{}".as_slice(),
                Some("unexpected media type"),
            ),
            (
                "200 OK",
                "Content-Type: audio/wav",
                b"not audio".as_slice(),
                Some("could not be verified"),
            ),
            (
                "302 Found",
                "Location: https://localhost:1/redirect-target",
                b"".as_slice(),
                Some("HTTP 302"),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = Url::parse(&format!(
                "https://localhost:{}/v1/speech/voice",
                listener.local_addr().unwrap().port()
            ))
            .unwrap();
            let request =
                speech_request(&client, &fixture(), endpoint, "olá", b"fixture-secret").unwrap();
            let output = directory.path().join(format!("{index}.wav"));
            thread::scope(|scope| {
                let config = Arc::clone(&config);
                let server = scope.spawn(move || {
                    let expires = Instant::now() + Duration::from_secs(5);
                    let socket = loop {
                        match listener.accept() {
                            Ok((socket, _)) => break socket,
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < expires =>
                                thread::sleep(Duration::from_millis(10)),
                            Err(error) => panic!("No HTTPS request arrived: {error}"),
                        }
                    };
                    socket.set_nonblocking(false).unwrap();
                    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let stream = StreamOwned::new(ServerConnection::new(config).unwrap(), socket);
                    let mut reader = BufReader::new(stream);
                    let mut headers = String::new();
                    loop {
                        let length = headers.len();
                        assert!(reader.read_line(&mut headers).unwrap() > 0);
                        if &headers[length..] == "\r\n" { break; }
                    }
                    assert!(headers.starts_with("POST /v1/speech/voice?"));
                    assert!(headers.contains("x-fixture-key: fixture-secret\r\n"));
                    let length = headers.lines().find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap().parse::<usize>().unwrap();
                    let mut json = vec![0; length];
                    reader.read_exact(&mut json).unwrap();
                    assert_eq!(serde_json::from_slice::<serde_json::Value>(&json).unwrap()["text"], "olá");
                    let socket = reader.get_mut();
                    write!(socket, "HTTP/1.1 {status}\r\n{extra}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                    socket.write_all(body).unwrap();
                    socket.flush().unwrap();
                });
                let result = execute_speech(
                    &client,
                    request,
                    &AudioFormat::Wav,
                    NamedTempFile::new_in(directory.path()).unwrap(),
                    &output,
                );
                server.join().unwrap();
                if let Some(expected) = expected_error {
                    assert!(result.unwrap_err().to_string().contains(expected));
                    assert!(!output.exists());
                } else {
                    result.unwrap();
                    assert_eq!(std::fs::read(&output).unwrap(), wav);
                }
            });
        }
    }
}
