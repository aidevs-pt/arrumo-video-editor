//! Anonymous usage counts and crash reports. Both are on by default and can be turned off in
//! Settings. Nothing is sent unless a release build was given the service keys at compile time
//! (`ARRUMO_USAGE_KEY`, `ARRUMO_CRASH_DSN`), so contributor builds never report anything.
//! Events carry an event name, a random install ID, a per-launch session ID, the app version and
//! the operating system: never content, file names, paths or account details.
//!
//! The desktop app sends usage through the Aptabase Tauri plugin (see [`set_usage_sink`]) and
//! crashes through the Sentry SDK, which checks [`crash_reports_allowed`]. Headless processes
//! (CLI, MCP, A2A) fall back to small adapters (`usage_request`, `crash_request`).
//!
//! Usage data also covers step timings and warnings ([`time_step`], [`warn`]), which the desktop
//! app sends through [`set_diagnostics`]; headless processes send none.

use std::{
    cell::Cell,
    collections::BTreeMap,
    error::Error,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::params;
use serde::Serialize;
use serde_json::{Value, json};

use crate::WorkspaceStore;

const USAGE_KEY: Option<&str> = option_env!("ARRUMO_USAGE_KEY");
const USAGE_ENDPOINT: Option<&str> = option_env!("ARRUMO_USAGE_ENDPOINT");
const CRASH_DSN: Option<&str> = option_env!("ARRUMO_CRASH_DSN");

/// Receives each allowed usage event with its properties, including the install ID.
type UsageSink = Box<dyn Fn(&str, Value) + Send + Sync>;
static USAGE_SINK: OnceLock<UsageSink> = OnceLock::new();
static CRASH_ALLOWED: AtomicBool = AtomicBool::new(false);
/// The random ID crash reports carry while they are on; see [`crash_report_id`].
static CRASH_ID: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);
static USAGE_ALLOWED: AtomicBool = AtomicBool::new(false);

/// Receives timings and warnings while the creator allows usage data. Names and attributes are
/// fixed values, such as a stage, a program's file name, a plugin ID or a round, never a title,
/// path, prompt or message from a provider.
pub trait Diagnostics: Send + Sync {
    /// Starts timing `name`, inside the timing already running on this thread, if any.
    fn start(
        &self,
        op: &'static str,
        name: &str,
        attributes: &[(&'static str, Value)],
    ) -> Box<dyn TimingEnd>;
    fn warn(&self, message: &'static str, attributes: &[(&'static str, Value)]);
}

pub trait TimingEnd {
    fn end(self: Box<Self>, ok: bool);
}

static DIAGNOSTICS: OnceLock<Box<dyn Diagnostics>> = OnceLock::new();

thread_local! {
    static OPEN_TIMINGS: Cell<u32> = const { Cell::new(0) };
}

/// Sends timings and warnings through `diagnostics`, such as the desktop app's crash reporter.
pub fn set_diagnostics(diagnostics: impl Diagnostics + 'static) {
    let _ = DIAGNOSTICS.set(Box::new(diagnostics));
}

/// Whether the creator currently allows usage data, for SDKs to check before sending.
pub fn usage_data_allowed() -> bool {
    USAGE_ALLOWED.load(Ordering::Relaxed)
}

/// Times a piece of work until [`Timing::finish`]; dropping it unfinished counts as a failure.
#[must_use]
pub struct Timing(Option<Box<dyn TimingEnd>>);

impl Timing {
    pub fn finish(mut self, ok: bool) {
        self.end(ok);
    }

    fn end(&mut self, ok: bool) {
        if let Some(end) = self.0.take() {
            OPEN_TIMINGS.with(|open| open.set(open.get().saturating_sub(1)));
            end.end(ok);
        }
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        self.end(false);
    }
}

/// Times a generation step. Call it only once the step is claimed, so idle polling sends nothing.
pub fn time_step(stage: &str, attributes: &[(&'static str, Value)]) -> Timing {
    time("generation.step", stage, attributes)
}

/// Times part of a step, such as a program run or an agent session. Outside a step it times
/// nothing, so checks that run on their own are not sent.
pub(crate) fn time_part(
    op: &'static str,
    name: &str,
    attributes: &[(&'static str, Value)],
) -> Timing {
    if OPEN_TIMINGS.with(Cell::get) == 0 {
        return Timing(None);
    }
    time(op, name, attributes)
}

fn time(op: &'static str, name: &str, attributes: &[(&'static str, Value)]) -> Timing {
    match DIAGNOSTICS.get() {
        Some(diagnostics) if usage_data_allowed() => {
            OPEN_TIMINGS.with(|open| open.set(open.get() + 1));
            Timing(Some(diagnostics.start(op, name, attributes)))
        }
        _ => Timing(None),
    }
}

/// Records a warning for diagnostics. The message is a fixed sentence; details go in attributes.
pub fn warn(message: &'static str, attributes: &[(&'static str, Value)]) {
    if let Some(diagnostics) = DIAGNOSTICS.get().filter(|_| usage_data_allowed()) {
        diagnostics.warn(message, attributes);
    }
}

/// The usage service key this build was given, if any.
pub fn usage_key() -> Option<&'static str> {
    USAGE_KEY
}

/// The crash service DSN this build was given, if any.
pub fn crash_dsn() -> Option<&'static str> {
    CRASH_DSN
}

/// Sends usage events through `sink` instead of the built-in adapter, such as the desktop app's
/// analytics plugin. Events still pass the creator's setting first.
pub fn set_usage_sink(sink: impl Fn(&str, Value) + Send + Sync + 'static) {
    let _ = USAGE_SINK.set(Box::new(sink));
}

/// Receives handled failures that the creator should not have hit, already redacted, as
/// `(area, message, details)`. Details name where it failed and why, such as the stage and the
/// underlying error.
type FailureSink = Box<dyn Fn(&FailureReport<'_>) + Send + Sync>;
static FAILURE_SINK: OnceLock<FailureSink> = OnceLock::new();

/// A handled failure as the crash reporter receives it, already masked.
pub struct FailureReport<'a> {
    /// The feature that failed, such as `generation`.
    pub area: &'a str,
    /// What failed and, when known, the error at the bottom of it, with names and numbers left
    /// out so the title reads the same each time the failure happens.
    pub title: String,
    /// Where and why it failed, such as the stage and the whole error chain.
    pub details: Vec<(&'a str, String)>,
    /// Groups reports by where and why they failed rather than by their wording.
    pub fingerprint: Vec<String>,
}

/// Sends handled failures through `sink`, such as the desktop app's crash reporter. They pass the
/// crash report setting first.
pub fn set_failure_sink(sink: impl Fn(&FailureReport<'_>) + Send + Sync + 'static) {
    let _ = FAILURE_SINK.set(Box::new(sink));
}

/// An error that needs no report from callers further up: it was already reported, or its run
/// was stopped or taken over, which is not a failure. It keeps the error's chain as text.
#[derive(Debug)]
pub struct Handled(String);

impl Handled {
    pub fn new(error: &(dyn std::error::Error + 'static)) -> Self {
        Self(error_chain(error))
    }
}

impl std::fmt::Display for Handled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Handled {}

/// Whether `error`, or an error it wraps, needs no report; see [`Handled`].
pub fn already_handled(error: &(dyn std::error::Error + 'static)) -> bool {
    crate::quiet_process::error_sources(error).any(|error| error.is::<Handled>())
}

/// The first line of the error at the bottom of `error`'s chain: what actually went wrong.
pub fn root_cause(error: &(dyn std::error::Error + 'static)) -> String {
    let root = crate::quiet_process::error_sources(error)
        .last()
        .map(ToString::to_string)
        .unwrap_or_default();
    root.lines().next().unwrap_or_default().to_owned()
}

/// Reports `error` with its whole chain and its root cause, unless it is [`Handled`].
pub fn report_error(
    area: &str,
    error: &(dyn std::error::Error + 'static),
    details: &[(&str, &str)],
) {
    if already_handled(error) {
        return;
    }
    let cause = error_chain(error);
    let root = root_cause(error);
    let mut details = details.to_vec();
    details.extend([("cause", cause.as_str()), ("root", root.as_str())]);
    report_failure_with(area, &error.to_string(), &details);
}

/// Reports a failure the app handled but the creator should not have hit, such as a render that
/// stopped, so it can be fixed. `area` names the feature. The report keeps the whole error, with
/// the home folder written as `~` (see [`mask_home`]); its first line is the report's title.
pub fn report_failure(area: &str, error: &dyn std::fmt::Display) {
    report_failure_with(area, error, &[]);
}

/// [`report_failure`] with details of where and why, such as the error chain and a program's
/// output, masked the same way.
pub fn report_failure_with(area: &str, error: &dyn std::fmt::Display, details: &[(&str, &str)]) {
    if !crash_reports_allowed() {
        return;
    }
    if let Some(sink) = FAILURE_SINK.get() {
        sink(&failure_report(area, error, details));
    }
}

/// The report [`report_failure_with`] sends: the masked error, its details, and a title and
/// fingerprint from the stage, the failed step and the root cause.
fn failure_report<'a>(
    area: &'a str,
    error: &dyn std::fmt::Display,
    details: &[(&'a str, &str)],
) -> FailureReport<'a> {
    let text = mask_home(&error.to_string());
    let title: String = text
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();
    let mut details: Vec<(&str, String)> = details
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| (*key, limited(&mask_home(value))))
        .collect();
    if text.trim() != title.trim() {
        details.push(("error", limited(&text)));
    }
    let detail = |key: &str| {
        details
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str())
    };
    let root = detail("root").map(report_pattern);
    let step = detail("cause")
        .and_then(|cause| cause.lines().next())
        .map(report_pattern)
        .unwrap_or_default();
    let fingerprint = vec![
        area.to_owned(),
        detail("stage")
            .or(detail("worker"))
            .unwrap_or_default()
            .to_owned(),
        step,
        root.clone().unwrap_or_else(|| report_pattern(&title)),
    ];
    let title = match root {
        // A root that is the failure itself would only repeat it.
        Some(root) if !root.is_empty() && root != report_pattern(&title) => {
            format!("{}: {root}", first_sentence(&title))
        }
        _ => title,
    };
    FailureReport {
        area,
        title,
        details,
        fingerprint,
    }
}

/// The first sentence of a message, without its full stop.
fn first_sentence(message: &str) -> &str {
    message
        .split_once(". ")
        .map_or(message, |(sentence, _)| sentence)
        .trim_end_matches('.')
}

/// A line of an error with what changes between occurrences left out: code locations, quoted
/// text, paths and addresses, and numbers. Reports that read the same this way share a cause.
pub(crate) fn report_pattern(line: &str) -> String {
    static LOCATION: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\s*\([\w./-]+\.rs:\d+\)").unwrap());
    static NUMBER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\d+").unwrap());
    let line = LOCATION.replace_all(line, "");
    let line = without_quotes(&line).replace("[redacted]", "*");
    let words: Vec<&str> = line
        .split_whitespace()
        .map(|word| {
            if ['/', '\\', '@', '*']
                .iter()
                .any(|mark| word.contains(*mark))
                || word.contains("://")
            {
                "*"
            } else {
                word
            }
        })
        .collect();
    NUMBER
        .replace_all(&words.join(" "), "N")
        .chars()
        .take(200)
        .collect()
}

/// The most a report field carries, below the crash service's limit for a single value.
const REPORT_FIELD_LIMIT: usize = 8000;

fn limited(text: &str) -> String {
    text.chars().take(REPORT_FIELD_LIMIT).collect()
}

/// Writes the creator's home folder, and any other user's home folder, as `~`, and email
/// addresses as `[email]`, so a report keeps every path and message needed to debug it, but not
/// the user names home folders are named after.
pub fn mask_home(text: &str) -> String {
    let homes: Vec<String> = ["HOME", "USERPROFILE"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|home| home.to_string_lossy().into_owned())
        .collect();
    mask_folders(text, &homes)
}

fn mask_folders(text: &str, homes: &[String]) -> String {
    let mut forms: Vec<String> = homes
        .iter()
        .map(|home| home.trim_end_matches(['/', '\\']).to_owned())
        .filter(|home| home.len() > 1)
        // Windows tools also write the folder with forward slashes or in lower case.
        .flat_map(|home| {
            let slashed = home.replace('\\', "/");
            [home.to_lowercase(), slashed.to_lowercase(), slashed, home]
        })
        .collect();
    forms.sort_by_key(|form| std::cmp::Reverse(form.len()));
    forms.dedup();
    let masked = forms.iter().fold(text.to_owned(), |masked, home| {
        masked.replace(home.as_str(), "~")
    });
    static HOME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"(?i)(?:\b[a-z]:[\\/]users[\\/]|/users/|/home/)[^\\/\s"']+"#).unwrap()
    });
    static EMAIL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"[\w.%+-]+@[\w-]+(?:\.[\w-]+)*\.[a-zA-Z]{2,}").unwrap()
    });
    let masked = HOME.replace_all(&masked, "~");
    let masked = EMAIL.replace_all(&masked, "[email]");
    without_provider_text(&masked)
}

/// Replaces what a provider wrote back, which [`provider_text`] marks, since it may repeat the
/// prompt it was given. The creator still sees it in the app.
fn without_provider_text(text: &str) -> String {
    static PROVIDER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"«[^»]*»?").unwrap());
    PROVIDER.replace_all(text, "«provider text»").into_owned()
}

/// Marks what a provider wrote back so reports leave it out; see [`without_provider_text`].
pub(crate) fn provider_text(text: &str) -> String {
    format!("«{}»", text.replace(['«', '»'], ""))
}

/// The error and each error that caused it, one per line, for a failure report.
pub fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut lines = vec![error.to_string()];
    let mut source = error.source();
    while let Some(cause) = source {
        lines.push(cause.to_string());
        source = cause.source();
    }
    lines.join("\n")
}

/// Redacts every line of a longer text, such as an error chain or tool output.
pub(crate) fn redact_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .map(|line| redact(&without_quotes(line)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replaces quoted text with `[redacted]`.
fn without_quotes(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut closing: Option<char> = None;
    for ch in message.chars() {
        match closing {
            Some(end) if ch == end => {
                closing = None;
                out.push_str("[redacted]");
            }
            Some(_) => {}
            None => match ch {
                '"' => closing = Some('"'),
                '“' => closing = Some('”'),
                '«' => closing = Some('»'),
                '\'' if !out.ends_with(|c: char| c.is_alphanumeric()) => closing = Some('\''),
                _ => out.push(ch),
            },
        }
    }
    if closing.is_some() {
        out.push_str("[redacted]");
    }
    out
}

/// Whether the creator currently allows crash reports, for crash SDKs to check before sending.
/// Kept up to date by [`WorkspaceStore::privacy_settings`] and its setter.
pub fn crash_reports_allowed() -> bool {
    CRASH_ALLOWED.load(Ordering::Relaxed)
}

/// A random ID that only crash reports carry, so reports from one install can be grouped. It is
/// not the usage install ID, and turning crash reports off replaces it. `None` until the store
/// opens or while crash reports are off.
pub fn crash_report_id() -> Option<String> {
    let id = CRASH_ID
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    id.clone().filter(|_| crash_reports_allowed())
}

/// Sets the crash report choice before the store is open, from the copy kept beside it.
pub fn allow_crash_reports(allowed: bool) {
    CRASH_ALLOWED.store(allowed, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    pub usage_data: bool,
    pub crash_reports: bool,
    /// Whether this build can send anything at all.
    pub usage_available: bool,
    pub crash_available: bool,
}

impl WorkspaceStore {
    pub fn privacy_settings(&self) -> rusqlite::Result<PrivacySettings> {
        self.0.query_row(
            "SELECT usage_data, crash_reports, crash_report_id FROM privacy_settings WHERE id = 1",
            [],
            |row| {
                let usage_data = row.get(0)?;
                let crash_reports = row.get(1)?;
                USAGE_ALLOWED.store(usage_data, Ordering::Relaxed);
                CRASH_ALLOWED.store(crash_reports, Ordering::Relaxed);
                *CRASH_ID
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = row.get(2)?;
                Ok(PrivacySettings {
                    usage_data,
                    crash_reports,
                    usage_available: USAGE_KEY.is_some(),
                    crash_available: CRASH_DSN.is_some(),
                })
            },
        )
    }

    pub fn set_privacy_settings(
        &self,
        usage_data: bool,
        crash_reports: bool,
    ) -> rusqlite::Result<PrivacySettings> {
        self.0.execute(
            // Turning crash reports off replaces their ID, so later reports start a new one.
            "UPDATE privacy_settings SET usage_data = ?1, crash_reports = ?2,
                 crash_report_id = CASE WHEN ?2 THEN crash_report_id
                                        ELSE lower(hex(randomblob(16))) END
             WHERE id = 1",
            params![usage_data, crash_reports],
        )?;
        self.privacy_settings()
    }

    fn install_id(&self) -> rusqlite::Result<String> {
        self.0.query_row(
            "SELECT install_id FROM privacy_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
    }

    /// Counts a new install once, on its first launch, rather than every launch.
    pub fn track_install(&self) {
        let first = self.0.execute(
            "UPDATE privacy_settings SET install_reported = 1 WHERE id = 1 AND install_reported = 0",
            [],
        );
        if first == Ok(1) {
            self.track_usage("app_installed", &[]);
        }
    }

    /// Counts one use of a feature, if the creator allows it and this build can send it.
    /// `props` must only hold coarse, non-identifying values such as a provider ID or a flag.
    pub fn track_usage(&self, event: &str, props: &[(&str, Value)]) {
        let (Some(key), Ok(settings), Ok(install)) =
            (USAGE_KEY, self.privacy_settings(), self.install_id())
        else {
            return;
        };
        if !settings.usage_data || !valid_event(event) {
            return;
        }
        let props: BTreeMap<&str, Value> = props
            .iter()
            .filter(|(_, value)| {
                value.is_boolean()
                    || value.is_number()
                    || value.as_str().is_some_and(|text| text.len() <= 64)
            })
            .cloned()
            .collect();
        if let Some(sink) = USAGE_SINK.get() {
            let mut props: serde_json::Map<String, Value> = props
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect();
            props.insert("install".into(), json!(install));
            sink(event, Value::Object(props));
            return;
        }
        let body = usage_request(event, &install, &props);
        let endpoint = USAGE_ENDPOINT.unwrap_or("https://eu.aptabase.com/api/v0/events");
        send(endpoint, vec![("App-Key".to_owned(), key.to_owned())], body);
    }
}

fn valid_event(event: &str) -> bool {
    !event.is_empty()
        && event.len() <= 48
        && event
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

fn session_id() -> &'static str {
    static SESSION: OnceLock<String> = OnceLock::new();
    SESSION.get_or_init(|| uuid::Uuid::new_v4().simple().to_string())
}

fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

fn timestamp() -> String {
    let seconds = now().as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The usage service's request body (Aptabase's event API).
fn usage_request(event: &str, install: &str, props: &BTreeMap<&str, Value>) -> Value {
    let mut props = props.clone();
    props.insert("install", json!(install));
    json!([{
        "timestamp": timestamp(),
        "sessionId": session_id(),
        "eventName": event,
        "systemProps": {
            "osName": std::env::consts::OS,
            "osVersion": "",
            "locale": "",
            "appVersion": env!("CARGO_PKG_VERSION"),
            "isDebug": cfg!(debug_assertions),
            "sdkVersion": "arrumo-telemetry@1",
        },
        "props": props,
    }])
}

/// A crash message as reports carry it: whole, with the home folder written as `~`, for crash
/// SDKs to apply before they send.
pub fn mask_report_text(message: &str) -> String {
    limited(&mask_home(message))
}

/// Removes anything that looks like a local path or address from a usage log, which carries no
/// file paths, for diagnostics SDKs to apply before they send.
pub fn redact_report_text(message: &str) -> String {
    redact(message)
}

/// Removes anything that looks like a local path or address from a crash message.
fn redact(message: &str) -> String {
    let first = message.lines().next().unwrap_or_default();
    first
        .split_whitespace()
        .map(|word| {
            if word.contains('/')
                || word.contains('\\')
                || word.contains('@')
                || word.contains("://")
            {
                "[redacted]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect()
}

/// The crash service's store endpoint and request (Sentry's store API), from a DSN of the form
/// `https://<key>@<host>/<project>`.
fn crash_request(dsn: &str, message: &str, location: &str) -> Option<(String, String, Value)> {
    let rest = dsn.strip_prefix("https://")?;
    let (key, rest) = rest.split_once('@')?;
    let (host, project) = rest.split_once('/')?;
    if key.is_empty() || host.is_empty() || !project.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let endpoint = format!("https://{host}/api/{project}/store/");
    let auth =
        format!("Sentry sentry_version=7, sentry_key={key}, sentry_client=arrumo-telemetry/1");
    let body = json!({
        "event_id": uuid::Uuid::new_v4().simple().to_string(),
        "timestamp": now().as_secs(),
        "platform": "native",
        "level": "fatal",
        "release": format!("arrumo@{}", env!("CARGO_PKG_VERSION")),
        "environment": if cfg!(debug_assertions) { "debug" } else { "release" },
        "tags": {"os": std::env::consts::OS, "session": session_id()},
        // An explicit address keeps the crash service from locating the sender by theirs.
        "user": {"id": crash_report_id(), "ip_address": "0.0.0.0"},
        "exception": {"values": [{"type": "panic", "value": mask_report_text(message), "module": location}]},
    });
    Some((endpoint, auth, body))
}

fn send(endpoint: &str, headers: Vec<(String, String)>, body: Value) {
    let endpoint = endpoint.to_owned();
    let _ = std::thread::Builder::new()
        .name("arrumo-telemetry".into())
        .spawn(move || {
            let _ = (|| -> Result<(), Box<dyn Error>> {
                let client = reqwest::blocking::Client::builder()
                    .https_only(true)
                    .timeout(Duration::from_secs(10))
                    .build()?;
                let mut request = client
                    .post(&endpoint)
                    .header("Content-Type", "application/json")
                    .body(body.to_string());
                for (name, value) in headers {
                    request = request.header(name, value);
                }
                request.send()?;
                Ok(())
            })();
        });
}

/// Reports panics while the creator allows crash reports. The setting is read when the panic
/// happens, from the database at `database`.
pub fn install_crash_reporter(database: std::path::PathBuf) {
    let Some(dsn) = CRASH_DSN else {
        return;
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let allowed = WorkspaceStore::open(&database)
            .and_then(|store| store.privacy_settings())
            .is_ok_and(|settings| settings.crash_reports);
        if allowed {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            // Only the source file inside Arrumo, never the machine's paths.
            let location = info
                .location()
                .map(|place| {
                    let file = place.file().rsplit(['/', '\\']).next().unwrap_or_default();
                    format!("{file}:{}", place.line())
                })
                .unwrap_or_default();
            if let Some((endpoint, auth, body)) = crash_request(dsn, &message, &location) {
                send(&endpoint, vec![("X-Sentry-Auth".to_owned(), auth)], body);
                // Give the report a moment to leave before the process ends.
                std::thread::sleep(Duration::from_millis(1500));
            }
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_reports_leave_out_quoted_names_and_paths() {
        assert_eq!(
            redact(&without_quotes(
                "Could not render \"Lower third\" from C:\\Users\\ana\\clip.mp4: drawtext failed"
            )),
            "Could not render [redacted] from [redacted] drawtext failed"
        );
        assert_eq!(
            without_quotes("Can't read 'brand.ttf' or «Mi vídeo»"),
            "Can't read [redacted] or [redacted]"
        );
        assert_eq!(without_quotes("Unclosed \"name"), "Unclosed [redacted]");
    }

    /// Held by tests that turn usage data off or rely on it being on.
    static USAGE_SETTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn privacy_settings_default_on_and_persist() {
        let _usage = USAGE_SETTING
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let store = WorkspaceStore::open(":memory:").unwrap();
        let settings = store.privacy_settings().unwrap();
        assert!(settings.usage_data && settings.crash_reports);
        assert_eq!(store.install_id().unwrap().len(), 32);
        let off = store.set_privacy_settings(false, true).unwrap();
        assert!(!off.usage_data && off.crash_reports);
        // Contributor builds have no keys, so tracking is a no-op.
        store.track_usage("app_installed", &[]);
    }

    #[test]
    fn failures_group_by_step_and_root_cause_not_by_wording() {
        let report = |cause: &str, root: &str| {
            failure_report(
                "generation",
                &"Final rendering failed. Check the accepted edit.\nRendering the cut (clip_render.rs:753)",
                &[("stage", "final_render"), ("cause", cause), ("root", root)],
            )
        };
        let first = report(
            "Rendering the cut (clip_render.rs:753)\nFFmpeg failed while rendering (exit status: 1): /Users/ana/a.mp4: Invalid data",
            "FFmpeg failed while rendering (exit status: 1): /Users/ana/a.mp4: Invalid data",
        );
        let again = report(
            "Rendering the cut (clip_render.rs:760)\nFFmpeg failed while rendering (exit status: 234): 'Holiday.mp4': Invalid data",
            "FFmpeg failed while rendering (exit status: 234): 'Holiday.mp4': Invalid data",
        );
        assert_eq!(first.fingerprint, again.fingerprint);
        assert_eq!(
            first.title,
            "Final rendering failed: FFmpeg failed while rendering (exit status: N): * Invalid data"
        );
        assert!(!again.title.contains("Holiday"));
        let other = report(
            "Rendering the cut (clip_render.rs:753)\nQuery returned no rows",
            "Query returned no rows",
        );
        assert_ne!(first.fingerprint, other.fingerprint);
        assert_eq!(
            other.title,
            "Final rendering failed: Query returned no rows"
        );
        // Without a root cause the title stays the failure's own first line.
        let plain = failure_report("backup", &"Could not restore 3 files", &[]);
        assert_eq!(plain.title, "Could not restore 3 files");
        assert_eq!(plain.fingerprint[3], "Could not restore N files");
        let single = failure_report(
            "background",
            &"Disk full",
            &[("cause", "Disk full"), ("root", "Disk full")],
        );
        assert_eq!(single.title, "Disk full");
    }

    #[test]
    fn a_handled_error_is_not_reported_again() {
        let handled: Box<dyn std::error::Error> =
            Box::new(Handled::new(&std::io::Error::other("disk full")));
        assert!(already_handled(&*handled));
        assert_eq!(handled.to_string(), "disk full");
        assert!(!already_handled(&std::io::Error::other("disk full")));
    }

    #[test]
    fn crash_reports_carry_their_own_id_and_lose_it_when_turned_off() {
        let _usage = USAGE_SETTING
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let store = WorkspaceStore::open(":memory:").unwrap();
        let id = |store: &WorkspaceStore| -> String {
            store
                .0
                .query_row("SELECT crash_report_id FROM privacy_settings", [], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        let first = id(&store);
        assert_eq!(first.len(), 32);
        assert_ne!(first, store.install_id().unwrap());
        store.set_privacy_settings(false, true).unwrap();
        assert_eq!(id(&store), first);
        assert_eq!(crash_report_id().as_deref(), Some(first.as_str()));
        store.set_privacy_settings(true, false).unwrap();
        assert!(crash_report_id().is_none());
        let renewed = store.set_privacy_settings(true, true).unwrap();
        assert!(renewed.crash_reports);
        assert_ne!(id(&store), first);
        assert_eq!(crash_report_id(), Some(id(&store)));
    }

    #[test]
    fn only_claimed_steps_and_the_work_inside_them_are_timed() {
        use std::cell::RefCell;
        thread_local! {
            static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        }
        struct Record;
        struct End(String);
        impl Diagnostics for Record {
            fn start(
                &self,
                op: &'static str,
                name: &str,
                _: &[(&'static str, Value)],
            ) -> Box<dyn TimingEnd> {
                SEEN.with(|seen| seen.borrow_mut().push(format!("start {op} {name}")));
                Box::new(End(name.to_owned()))
            }
            fn warn(&self, message: &'static str, _: &[(&'static str, Value)]) {
                SEEN.with(|seen| seen.borrow_mut().push(message.to_owned()));
            }
        }
        impl TimingEnd for End {
            fn end(self: Box<Self>, ok: bool) {
                SEEN.with(|seen| seen.borrow_mut().push(format!("end {} {ok}", self.0)));
            }
        }
        let _usage = USAGE_SETTING
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        set_diagnostics(Record);
        USAGE_ALLOWED.store(true, Ordering::Relaxed);
        time_part("process", "arrumo-ffprobe", &[]).finish(true);
        let step = time_step("preview", &[]);
        time_part("process", "arrumo-ffmpeg", &[]).finish(false);
        warn("A generation step failed", &[]);
        drop(step);
        time_part("process", "arrumo-ffprobe", &[]).finish(true);
        USAGE_ALLOWED.store(false, Ordering::Relaxed);
        time_step("script", &[]).finish(true);
        warn("A generation step failed", &[]);
        USAGE_ALLOWED.store(true, Ordering::Relaxed);
        assert_eq!(
            SEEN.with(|seen| seen.take()),
            [
                "start generation.step preview",
                "start process arrumo-ffmpeg",
                "end arrumo-ffmpeg false",
                "A generation step failed",
                "end preview false",
            ]
        );
    }

    #[test]
    fn a_new_install_is_counted_once() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let reported = || -> bool {
            store
                .0
                .query_row("SELECT install_reported FROM privacy_settings", [], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        assert!(!reported());
        store.track_install();
        assert!(reported());
        store.track_install();
        assert!(reported());
    }

    #[test]
    fn an_update_is_not_counted_as_an_install() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.sqlite");
        WorkspaceStore::open(&path)
            .unwrap()
            .0
            .execute_batch(
                "ALTER TABLE privacy_settings DROP COLUMN install_reported;
                 ALTER TABLE privacy_settings DROP COLUMN crash_report_id;
                 DROP TRIGGER generation_activity_transition;
                 ALTER TABLE generation_activity DROP COLUMN stage;
                 ALTER TABLE agent_logs DROP COLUMN round; ALTER TABLE agent_sessions DROP COLUMN round;
                 DROP TABLE profile_formats_off; DROP TABLE storage_settings;
                 PRAGMA user_version = 68;",
            )
            .unwrap();
        let store = WorkspaceStore::open(&path).unwrap();
        let reported: bool = store
            .0
            .query_row("SELECT install_reported FROM privacy_settings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(reported);
    }

    #[test]
    fn reports_keep_the_whole_message_without_the_home_folder() {
        let homes = ["/Users/ana".to_owned(), "C:\\Users\\Ana Rita\\".to_owned()];
        assert_eq!(
            mask_folders(
                "could not open /Users/ana/Movies/clip.mp4\nffmpeg -i C:\\Users\\Ana Rita\\a.mp4 c:/users/ana rita/b.mp4",
                &homes
            ),
            "could not open ~/Movies/clip.mp4\nffmpeg -i ~\\a.mp4 ~/b.mp4"
        );
        assert_eq!(mask_folders("/ stays", &["/".to_owned()]), "/ stays");
        assert_eq!(
            mask_folders(
                "at /home/bob/x and D:\\Users\\carl\\y and /Users/dee for dee@example.com",
                &[]
            ),
            "at ~/x and ~\\y and ~ for [email]"
        );
        assert_eq!(
            mask_folders(
                &format!("HTTP 400: {}", provider_text("Prompt «a cat» was rejected")),
                &[]
            ),
            "HTTP 400: «provider text»"
        );
        assert_eq!(limited(&"x".repeat(9000)).len(), REPORT_FIELD_LIMIT);
    }

    #[test]
    fn requests_follow_the_crash_service_format() {
        assert_eq!(
            redact("could not open /home/ana/Videos/secret.mp4 for ana@example.com\nmore"),
            "could not open [redacted] for [redacted]"
        );
        let (endpoint, auth, body) = crash_request(
            "https://abc123@o1.ingest.example.io/42",
            "boom at C:\\Users\\x",
            "main.rs:9",
        )
        .unwrap();
        assert_eq!(endpoint, "https://o1.ingest.example.io/api/42/store/");
        assert!(auth.contains("sentry_key=abc123"));
        assert_eq!(body["exception"]["values"][0]["value"], "boom at ~");
        assert!(crash_request("http://k@h/1", "x", "y").is_none());
        let usage = usage_request(
            "video_queued",
            "f".repeat(32).as_str(),
            &BTreeMap::from([("autopilot", json!(true))]),
        );
        assert_eq!(usage[0]["eventName"], "video_queued");
        assert_eq!(usage[0]["props"]["autopilot"], true);
        assert!(timestamp().ends_with('Z') && timestamp().len() == 20);
        assert!(valid_event("backup_exported") && !valid_event("Bad Event"));
    }
}
