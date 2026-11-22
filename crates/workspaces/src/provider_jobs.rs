//! Asynchronous provider jobs described by SDK descriptors: read the submitted job's status
//! address on the provider's host, poll it with the credential, then download the finished file
//! from its HTTPS address without the credential.

use std::{
    io::{self, Read},
    time::{Duration, Instant},
};

use reqwest::{
    blocking::Client,
    header::{HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;

const MAX_STATUS_BYTES: u64 = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Submission {
    /// JSON pointer to the job's status address, which must be on the endpoint's host.
    pub status_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct JobStatus {
    pub state: String,
    pub completed: Vec<String>,
    pub failed: Vec<String>,
    /// JSON pointer to the finished file's HTTPS address (`video` or `image` in descriptors).
    #[serde(rename = "video", alias = "image")]
    pub output: String,
    #[serde(default)]
    pub error: Option<String>,
}

impl JobStatus {
    pub(crate) fn valid(&self, submission: &Submission) -> bool {
        let pointer = |value: &str| value.starts_with('/') && value.len() <= 128;
        let states = |list: &[String]| {
            !list.is_empty()
                && list.len() <= 8
                && list.iter().all(|state| (1..=32).contains(&state.len()))
        };
        pointer(&submission.status_url)
            && pointer(&self.state)
            && pointer(&self.output)
            && self.error.as_deref().is_none_or(pointer)
            && states(&self.completed)
            && states(&self.failed)
    }
}

pub(crate) fn host(url: &str) -> Option<&str> {
    url.strip_prefix("https://")?
        .split('/')
        .next()
        .filter(|host| !host.is_empty())
}

pub(crate) fn safe_https(url: &str) -> bool {
    host(url).is_some() && !url.contains(['#', '@', ' ']) && !url.contains("/../")
}

/// A provider's short reason, such as `not_enough_credits`, safe to show.
pub(crate) fn provider_reason(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(120).collect()
}

pub(crate) fn read_json(response: reqwest::blocking::Response, what: &str) -> io::Result<Value> {
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
            "The provider returned HTTP {code} for the {what}{reason}"
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

/// Polls a submitted job until it completes, fails or times out, then downloads its output.
#[allow(clippy::too_many_arguments)]
pub(crate) fn await_output(
    client: &Client,
    submitted: &Value,
    endpoint: &str,
    credential: (&HeaderName, &HeaderValue),
    submission: &Submission,
    status: &JobStatus,
    (poll_seconds, timeout_seconds): (u64, u64),
    max_bytes: u64,
    what: &str,
) -> io::Result<Vec<u8>> {
    let status_url = submitted
        .pointer(&submission.status_url)
        .and_then(Value::as_str)
        .filter(|url| safe_https(url) && host(url) == host(endpoint))
        .ok_or_else(|| {
            io::Error::other(format!(
                "The {what} provider gave no status address on its own host"
            ))
        })?
        .to_owned();
    let started = Instant::now();
    let output_url = loop {
        std::thread::sleep(Duration::from_secs(poll_seconds));
        if started.elapsed() > Duration::from_secs(timeout_seconds) {
            return Err(io::Error::other(format!(
                "The {what} provider did not finish in time"
            )));
        }
        let answer = read_json(
            client
                .get(&status_url)
                .header(credential.0.clone(), credential.1.clone())
                .send()
                .map_err(|_| io::Error::other(format!("The {what} status request failed")))?,
            &format!("{what} status"),
        )?;
        let state = answer
            .pointer(&status.state)
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status.completed.iter().any(|done| done == state) {
            break answer
                .pointer(&status.output)
                .and_then(Value::as_str)
                .filter(|url| safe_https(url))
                .ok_or_else(|| {
                    io::Error::other(format!("The finished {what} has no HTTPS address"))
                })?
                .to_owned();
        }
        if status.failed.iter().any(|failed| failed == state) {
            let reason = status
                .error
                .as_deref()
                .and_then(|pointer| answer.pointer(pointer)?.as_str())
                .map_or_else(|| provider_reason(state), provider_reason);
            return Err(io::Error::other(format!(
                "The {what} provider stopped: {reason}"
            )));
        }
    };
    // The output is fetched from wherever the provider stores it, without the credential.
    let download = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(3))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| io::Error::other("Could not initialize the download transport"))?
        .get(&output_url)
        .send()
        .map_err(|_| io::Error::other(format!("The finished {what} could not be downloaded")))?;
    if !download.status().is_success() {
        return Err(io::Error::other(format!(
            "The {what} download returned HTTP {}",
            download.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    download
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io::Error::other(format!("The finished {what} could not be downloaded")))?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(io::Error::other(format!("The {what} size is invalid")));
    }
    Ok(bytes)
}
