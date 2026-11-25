//! Dubbing by a trusted provider plugin's `dubbing.json` descriptor: the sound of a workspace
//! asset is uploaded as a job, polled until the provider finishes, and the dub for one language
//! is stored as a new audio asset.

use crate::IoStep;
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};

use reqwest::{
    Url,
    blocking::{Client, RequestBuilder},
    header::{HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    PluginSettingKind, PluginSettingScope, WorkspaceStore,
    audio_transform::{audio_extension, provider_reason, soundtrack_mp3, valid_endpoint},
    plugin_catalog::{BUNDLED, bundled_manifests},
    speech_synthesis::{speech_client_builder, valid_key},
};

const MAX_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DubbingAdapter {
    schema_version: u32,
    plugin_id: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    create: CreateStep,
    status: StatusStep,
    result: ResultStep,
    languages: Vec<String>,
    max_input_seconds: u32,
    max_input_bytes: u64,
    max_wait_seconds: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateStep {
    endpoint: String,
    file_field: String,
    language_field: String,
    fields: BTreeMap<String, String>,
    id_field: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusStep {
    endpoint: String,
    status_field: String,
    done: Vec<String>,
    failed: Vec<String>,
    error_field: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResultStep {
    endpoint: String,
}

/// A provider that dubs, with the languages it dubs into.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DubbingProvider {
    pub provider: String,
    pub languages: Vec<String>,
    pub max_input_seconds: u32,
}

fn invalid_adapter() -> io::Error {
    io::Error::other("Invalid dubbing adapter")
}

/// Fills `{id}` and `{language}` path segments; any other braces make the template invalid.
fn fill(template: &str, id: &str, language: &str) -> io::Result<Url> {
    let (base, rest) = template
        .strip_prefix("https://")
        .and_then(|raw| raw.split_once('/'))
        .ok_or_else(invalid_adapter)?;
    let mut url = Url::parse(&format!("https://{base}/")).map_err(|_| invalid_adapter())?;
    {
        let mut segments = url.path_segments_mut().map_err(|_| invalid_adapter())?;
        segments.pop_if_empty();
        for part in rest.split('/') {
            segments.push(match part {
                "{id}" => id,
                "{language}" => language,
                literal if !literal.contains(['{', '}']) => literal,
                _ => return Err(invalid_adapter()),
            });
        }
    }
    Ok(url)
}

fn valid_template(template: &str, needs: &[&str]) -> bool {
    needs.iter().all(|placeholder| {
        template
            .split('/')
            .filter(|part| part == placeholder)
            .count()
            == 1
    }) && valid_endpoint(
        &template
            .replace("{id}", "id")
            .replace("{language}", "language"),
    )
}

fn valid_code(code: &str) -> bool {
    (2..=12).contains(&code.len())
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn dubbing_adapter(plugin_id: &str) -> io::Result<Option<DubbingAdapter>> {
    let Some((manifest, source)) = bundled_manifests()
        .iter()
        .zip(BUNDLED)
        .find(|(manifest, _)| manifest.id == plugin_id)
    else {
        return Ok(None);
    };
    let Some(bytes) = source.dubbing else {
        return Ok(None);
    };
    let adapter: DubbingAdapter = serde_json::from_slice(bytes).map_err(|_| invalid_adapter())?;
    let has_secret = manifest.settings.iter().flatten().any(|setting| {
        setting.id == adapter.credential_setting
            && setting.kind == PluginSettingKind::Secret
            && setting.scope == PluginSettingScope::Workspace
    });
    let create = &adapter.create;
    let status = &adapter.status;
    let ok = adapter.schema_version == 1
        && adapter.plugin_id == manifest.id
        && has_secret
        && manifest.capabilities.contains_key("dubbing")
        && matches!(adapter.credential_prefix.as_str(), "" | "Bearer ")
        && valid_endpoint(&create.endpoint)
        && [&create.file_field, &create.language_field, &create.id_field]
            .into_iter()
            .all(|key| valid_key(key))
        && create.file_field != create.language_field
        && !create.fields.contains_key(&create.file_field)
        && !create.fields.contains_key(&create.language_field)
        && create.fields.len() <= 16
        && create
            .fields
            .iter()
            .all(|(key, value)| valid_key(key) && value.chars().count() <= 256)
        && valid_template(&status.endpoint, &["{id}"])
        && !status.endpoint.contains("{language}")
        && valid_key(&status.status_field)
        && status.error_field.as_deref().is_none_or(valid_key)
        && !status.done.is_empty()
        && status
            .done
            .iter()
            .chain(&status.failed)
            .all(|value| (1..=64).contains(&value.len()))
        && valid_template(&adapter.result.endpoint, &["{id}", "{language}"])
        && (1..=200).contains(&adapter.languages.len())
        && adapter.languages.iter().all(|code| valid_code(code))
        && (1..=7200).contains(&adapter.max_input_seconds)
        && (1..=1024 * 1024 * 1024).contains(&adapter.max_input_bytes)
        && (60..=7200).contains(&adapter.max_wait_seconds);
    if !ok {
        return Err(invalid_adapter());
    }
    Ok(Some(adapter))
}

fn authorize(
    request: RequestBuilder,
    adapter: &DubbingAdapter,
    secret: &[u8],
) -> io::Result<RequestBuilder> {
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
        .map_err(|_| io::Error::other("Invalid credential format"))?;
    value.set_sensitive(true);
    Ok(request.header(header, value))
}

fn send(request: RequestBuilder) -> io::Result<reqwest::blocking::Response> {
    let response = request
        .send()
        .map_err(|_| io::Error::other("The request to the provider failed"))?;
    let status = response.status().as_u16();
    match status {
        200..=299 => Ok(response),
        401 | 403 => Err(io::Error::other(format!(
            "The provider refused the key for dubbing (HTTP {status}). Check that the key is allowed to dub and that your plan includes it."
        ))),
        402 => Err(io::Error::other(
            "The provider says your plan doesn't include dubbing or is out of credits (HTTP 402). Upgrade the plan or add credits, then retry.",
        )),
        _ => {
            let reason = provider_reason(response);
            Err(io::Error::other(format!(
                "The provider returned HTTP {status}{}",
                reason.map(|text| format!(": {text}")).unwrap_or_default()
            )))
        }
    }
}

fn json(response: reqwest::blocking::Response) -> io::Result<Value> {
    let mut body = Vec::new();
    response
        .take(1024 * 1024)
        .read_to_end(&mut body)
        .map_err(|_| io::Error::other("Could not read the provider's answer"))?;
    serde_json::from_slice(&body).map_err(|_| io::Error::other("The provider's answer is not JSON"))
}

fn valid_job_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// What a status answer says: still working, finished, or failed with the provider's reason.
#[derive(Debug, PartialEq)]
enum JobState {
    Working,
    Done,
    Failed(String),
}

fn job_state(step: &StatusStep, answer: &Value) -> JobState {
    let status = answer
        .get(&step.status_field)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if step.done.iter().any(|value| value == status) {
        return JobState::Done;
    }
    let error = step
        .error_field
        .as_ref()
        .and_then(|field| answer.get(field))
        .and_then(Value::as_str)
        .map(|text| {
            text.chars()
                .filter(|c| !c.is_control())
                .take(240)
                .collect::<String>()
        })
        .filter(|text| !text.trim().is_empty());
    if step.failed.iter().any(|value| value == status) || error.is_some() {
        return JobState::Failed(error.unwrap_or_else(|| status.to_owned()));
    }
    JobState::Working
}

impl WorkspaceStore {
    /// The dubbing providers on for this workspace.
    pub fn dubbing_providers(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<DubbingProvider>, Box<dyn std::error::Error>> {
        let mut listed = Vec::new();
        for entry in self
            .capability_index(Some(workspace_id))?
            .get("dubbing")
            .into_iter()
            .flatten()
        {
            if let Some(adapter) = dubbing_adapter(&entry.id)? {
                listed.push(DubbingProvider {
                    provider: entry.id.clone(),
                    languages: adapter.languages,
                    max_input_seconds: adapter.max_input_seconds,
                });
            }
        }
        Ok(listed)
    }

    /// Dubs the sound of workspace asset `asset_id` into `language` with the first dubbing
    /// provider (or `provider`), waits for the provider, and adds the dub as a new labelled audio
    /// asset whose notes name the source. Returns the new asset ID and the provider used.
    pub fn dub_asset(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_id: i64,
        language: &str,
        provider: Option<&str>,
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
            .get("dubbing")
            .map(|providers| providers.iter().map(|entry| entry.id.clone()).collect())
            .unwrap_or_default();
        let plugin = match provider {
            Some(id) if offered.iter().any(|entry| entry == id) => id.to_owned(),
            Some(_) => return Err(io::Error::other("That provider does not dub").into()),
            None => self
                .capture_provider_orders(workspace_id, &BTreeMap::new())?
                .get("dubbing")
                .and_then(|order| order.effective.first().cloned())
                .or_else(|| offered.first().cloned())
                .ok_or_else(|| {
                    io::Error::other("Turn on a dubbing provider in Settings › Services and keys")
                })?,
        };
        let adapter = dubbing_adapter(&plugin)?
            .ok_or_else(|| io::Error::other("That provider does not dub"))?;
        if !adapter.languages.iter().any(|code| code == language) {
            return Err(io::Error::other(format!(
                "{plugin} dubs into {}",
                adapter.languages.join(", ")
            ))
            .into());
        }
        let secret = self
            .provider_secret(&plugin, &adapter.credential_setting, workspace_id)?
            .ok_or_else(|| {
                io::Error::other("Add the provider's key in Settings › Services and keys")
            })?;

        let scratch = tempfile::tempdir()?;
        let input = scratch.path().join("source.mp3");
        soundtrack_mp3(
            &crate::asset_path(&crate::asset_root(database), workspace_id, asset_id),
            &input,
        )?;
        let seconds = crate::source_transcripts::media_duration(&input)?;
        if seconds > f64::from(adapter.max_input_seconds) {
            return Err(io::Error::other(format!(
                "This provider dubs up to {} seconds; the sound is {} seconds long. Trim it first",
                adapter.max_input_seconds,
                seconds.ceil()
            ))
            .into());
        }
        let bytes = std::fs::read(&input).io_step("Reading a file")?;
        if bytes.len() as u64 > adapter.max_input_bytes {
            return Err(io::Error::other("The sound is too large for this provider").into());
        }

        let client: Client = speech_client_builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| io::Error::other("Could not initialize dubbing transport"))?;
        let mut form = reqwest::blocking::multipart::Form::new()
            .text(adapter.create.language_field.clone(), language.to_owned());
        for (key, value) in &adapter.create.fields {
            form = form.text(key.clone(), value.clone());
        }
        form = form.part(
            adapter.create.file_field.clone(),
            reqwest::blocking::multipart::Part::bytes(bytes)
                .file_name("source.mp3")
                .mime_str("audio/mpeg")
                .map_err(|_| io::Error::other("Could not encode the sound"))?,
        );
        let created = json(send(authorize(
            client
                .post(Url::parse(&adapter.create.endpoint).map_err(|_| invalid_adapter())?)
                .multipart(form),
            &adapter,
            &secret,
        )?)?)?;
        let job = created
            .get(&adapter.create.id_field)
            .and_then(Value::as_str)
            .filter(|id| valid_job_id(id))
            .ok_or_else(|| io::Error::other("The provider did not start a dubbing job"))?
            .to_owned();

        let started = Instant::now();
        loop {
            std::thread::sleep(POLL_INTERVAL);
            let answer = json(send(authorize(
                client.get(fill(&adapter.status.endpoint, &job, language)?),
                &adapter,
                &secret,
            )?)?)?;
            match job_state(&adapter.status, &answer) {
                JobState::Done => break,
                JobState::Failed(reason) => {
                    return Err(io::Error::other(format!(
                        "The provider could not dub it: {reason}"
                    ))
                    .into());
                }
                JobState::Working
                    if started.elapsed() > Duration::from_secs(adapter.max_wait_seconds.into()) =>
                {
                    return Err(io::Error::other(format!(
                        "The provider is still dubbing after {} minutes. Its job is {job}",
                        adapter.max_wait_seconds / 60
                    ))
                    .into());
                }
                JobState::Working => {}
            }
        }

        let response = send(authorize(
            client.get(fill(&adapter.result.endpoint, &job, language)?),
            &adapter,
            &secret,
        )?)?;
        let mut output = Vec::new();
        response
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .map_err(|_| io::Error::other("Could not read the dub"))?;
        if output.len() as u64 > MAX_OUTPUT_BYTES {
            return Err(io::Error::other("The dub is too large").into());
        }
        let extension = audio_extension(&output)
            .or_else(|| (output.get(4..8) == Some(b"ftyp")).then_some("mp4"))
            .ok_or_else(|| io::Error::other("The provider did not return audio or video"))?;
        let stem = Path::new(&source.name)
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
            .take(60)
            .collect::<String>();
        let stem = stem.trim_matches('-');
        let file = scratch.path().join(format!(
            "{}-dub-{language}.{extension}",
            if stem.is_empty() { "audio" } else { stem }
        ));
        std::fs::write(&file, &output).io_step("Writing a file")?;
        crate::source_transcripts::media_duration(&file)
            .map_err(|_| io::Error::other("The dub could not be read"))?;

        let mut writer = WorkspaceStore::open(database)?;
        let asset = writer
            .import_asset(&crate::asset_root(database), workspace_id, &file)
            .map_err(|error| io::Error::other(error.to_string()))?;
        writer.set_asset_notes(
            workspace_id,
            asset.id,
            &format!("{} dubbed into {language} by {plugin}.", source.name),
        )?;
        writer.set_asset_organization(
            workspace_id,
            asset.id,
            crate::AssetOrganization {
                group: None,
                labels: vec!["Dub".to_owned()],
            },
        )?;
        Ok((asset.id, plugin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> StatusStep {
        serde_json::from_value(serde_json::json!({
            "endpoint": "https://api.example.com/v1/dubbing/{id}", "statusField": "status",
            "done": ["dubbed"], "failed": ["failed"], "errorField": "error"
        }))
        .unwrap()
    }

    #[test]
    fn templates_fill_only_their_segments() {
        assert_eq!(
            fill(
                "https://api.example.com/v1/dubbing/{id}/audio/{language}",
                "a/b",
                "es"
            )
            .unwrap()
            .as_str(),
            "https://api.example.com/v1/dubbing/a%2Fb/audio/es"
        );
        assert!(fill("https://api.example.com/v1/{other}", "a", "es").is_err());
        assert!(valid_template(
            "https://api.example.com/v1/dubbing/{id}",
            &["{id}"]
        ));
        assert!(!valid_template(
            "https://api.example.com/v1/dubbing",
            &["{id}"]
        ));
        assert!(!valid_template("http://api.example.com/v1/{id}", &["{id}"]));
    }

    #[test]
    fn reads_the_job_state() {
        let step = status();
        assert_eq!(
            job_state(&step, &serde_json::json!({"status": "dubbing"})),
            JobState::Working
        );
        assert_eq!(
            job_state(&step, &serde_json::json!({"status": "dubbed"})),
            JobState::Done
        );
        assert_eq!(
            job_state(
                &step,
                &serde_json::json!({"status": "failed", "error": "No speech"})
            ),
            JobState::Failed("No speech".into())
        );
        assert_eq!(
            job_state(
                &step,
                &serde_json::json!({"status": "dubbing", "error": null})
            ),
            JobState::Working
        );
        assert!(valid_job_id("21m00Tcm4TlvDq8ikWAM"));
        assert!(!valid_job_id("../x"));
    }

    #[test]
    fn the_bundled_elevenlabs_descriptor_is_accepted() {
        if bundled_manifests()
            .iter()
            .any(|manifest| manifest.id == "elevenlabs")
        {
            let adapter = dubbing_adapter("elevenlabs").unwrap().unwrap();
            assert!(adapter.languages.iter().any(|code| code == "pt"));
        }
    }
}
