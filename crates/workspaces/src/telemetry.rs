//! Anonymous usage counts and crash reports. Both are on by default and can be turned off in
//! Settings. Nothing is sent unless a release build was given the service keys at compile time
//! (`ARRUMO_USAGE_KEY`, `ARRUMO_CRASH_DSN`), so contributor builds never report anything.
//! Events carry an event name, a random install ID, a per-launch session ID, the app version and
//! the operating system: never content, file names, paths or account details.
//!
//! The desktop app sends usage through the Aptabase Tauri plugin (see [`set_usage_sink`]) and
//! crashes through the Sentry SDK, which checks [`crash_reports_allowed`]. Headless processes
//! (CLI, MCP, A2A) fall back to small adapters (`usage_request`, `crash_request`).

use std::{
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
type FailureSink = Box<dyn Fn(&str, &str, &[(&str, String)]) + Send + Sync>;
static FAILURE_SINK: OnceLock<FailureSink> = OnceLock::new();

/// Sends handled failures through `sink`, such as the desktop app's crash reporter. They pass the
/// crash report setting first.
pub fn set_failure_sink(sink: impl Fn(&str, &str, &[(&str, String)]) + Send + Sync + 'static) {
    let _ = FAILURE_SINK.set(Box::new(sink));
}

/// Reports a failure the app handled but the creator should not have hit, such as a render that
/// stopped, so it can be fixed. `area` names the feature. The message keeps its first line,
/// without paths, addresses or quoted text, where names and titles usually appear.
pub fn report_failure(area: &str, error: &dyn std::fmt::Display) {
    report_failure_with(area, error, &[]);
}

/// [`report_failure`] with details of where and why, redacted the same way line by line.
pub fn report_failure_with(area: &str, error: &dyn std::fmt::Display, details: &[(&str, &str)]) {
    if !crash_reports_allowed() {
        return;
    }
    if let Some(sink) = FAILURE_SINK.get() {
        let details: Vec<(&str, String)> = details
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(key, value)| (*key, redact_lines(value)))
            .collect();
        sink(area, &redact(&without_quotes(&error.to_string())), &details);
    }
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
fn redact_lines(text: &str) -> String {
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
            "SELECT usage_data, crash_reports FROM privacy_settings WHERE id = 1",
            [],
            |row| {
                let crash_reports = row.get(1)?;
                CRASH_ALLOWED.store(crash_reports, Ordering::Relaxed);
                Ok(PrivacySettings {
                    usage_data: row.get(0)?,
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
            "UPDATE privacy_settings SET usage_data = ?1, crash_reports = ?2 WHERE id = 1",
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

/// Removes anything that looks like a local path or address from a crash message, for crash
/// SDKs to apply before they send.
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
        "exception": {"values": [{"type": "panic", "value": redact(message), "module": location}]},
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

    #[test]
    fn privacy_settings_default_on_and_persist() {
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
                 DROP TRIGGER generation_activity_transition;
                 ALTER TABLE generation_activity DROP COLUMN stage;
                 ALTER TABLE agent_logs DROP COLUMN round; ALTER TABLE agent_sessions DROP COLUMN round;
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
    fn requests_carry_no_paths_or_content() {
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
        assert_eq!(
            body["exception"]["values"][0]["value"],
            "boom at [redacted]"
        );
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
