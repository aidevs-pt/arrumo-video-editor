//! Crash and error reports through Sentry, for Rust panics and errors and for the web frontend,
//! plus step timings and warnings ([`SentryDiagnostics`]). Builds without `ARRUMO_CRASH_DSN` start
//! nothing. Reports are dropped while the creator has crash reports turned off, timings and
//! warnings while usage data is off, and none of them carry personal data. Native crash dumps are not
//! collected: they hold raw memory that could include the creator's content.

use std::{borrow::Cow, path::Path, sync::Arc, time::Duration};

use serde_json::Value;

/// Keeps the crash reporter running; drop it to flush and stop.
pub struct CrashReports(#[allow(dead_code)] Option<sentry::ClientInitGuard>);

pub fn start() -> (CrashReports, Option<tauri_plugin_sentry::Sentry>) {
    let Some(dsn) = arrumo_workspaces::crash_dsn() else {
        return (CrashReports(None), None);
    };
    let plugin = tauri_plugin_sentry::Sentry::new();
    let mut options = options();
    options.transport = Some(Arc::new(GuardedTransports));
    let options = options.add_integration(plugin.clone());
    let guard = sentry::init((dsn, options));
    (CrashReports(Some(guard)), Some(plugin))
}

fn options() -> sentry::ClientOptions {
    // Steps start rarely and only once claimed, so every one can be timed.
    let mut options = sentry::ClientOptions::new().traces_sample_rate(1.0);
    options.release = sentry::release_name!();
    options.environment = Some(Cow::Borrowed(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }));
    options.send_default_pii = false;
    // A stack shows which code reported a failure; symbols come from the debug files uploaded
    // for each release.
    options.attach_stacktrace = true;
    // Sessions are usage data, which the usage service counts under its own setting.
    options.auto_session_tracking = false;
    options.before_send = Some(std::sync::Arc::new(|mut event| {
        if !arrumo_workspaces::crash_reports_allowed() {
            return None;
        }
        scrub(&mut event);
        Some(event)
    }));
    options.before_send_log = Some(Arc::new(|mut log| {
        if !arrumo_workspaces::usage_data_allowed() {
            return None;
        }
        log.body = arrumo_workspaces::redact_report_text(&log.body);
        log.attributes.retain(|key, _| {
            key.starts_with("sentry.") || DIAGNOSTIC_ATTRIBUTES.contains(&key.as_str())
        });
        Some(log)
    }));
    options
}

/// Leaves out who and where: no user or machine name, local paths cut to file names, and path
/// or address words removed from error messages.
fn scrub(event: &mut sentry::protocol::Event<'static>) {
    event.user = None;
    event.server_name = None;
    // Breadcrumbs can quote console text and page addresses with titles in them.
    event.breadcrumbs = Default::default();
    event.request = None;
    if let Some(message) = &mut event.message {
        *message = arrumo_workspaces::redact_report_text(message);
    }
    for exception in &mut event.exception.values {
        if let Some(value) = &mut exception.value {
            *value = arrumo_workspaces::redact_report_text(value);
        }
    }
    for image in &mut event.debug_meta.to_mut().images {
        if let sentry::protocol::DebugImage::Symbolic(image) = image {
            image.name = file_name(&image.name);
        }
    }
}

/// The attributes [`arrumo_workspaces::warn`] callers pass; the SDK's own, such as the machine
/// name, are left out.
const DIAGNOSTIC_ATTRIBUTES: &[&str] = &["stage", "agent", "round"];

/// Sends step timings as Sentry traces and warnings as Sentry logs.
pub struct SentryDiagnostics;

impl arrumo_workspaces::Diagnostics for SentryDiagnostics {
    fn start(
        &self,
        op: &'static str,
        name: &str,
        attributes: &[(&'static str, Value)],
    ) -> Box<dyn arrumo_workspaces::TimingEnd> {
        let parent = sentry::configure_scope(|scope| scope.get_span());
        let span: sentry::TransactionOrSpan = match &parent {
            Some(parent) => parent.start_child(op, name).into(),
            None => sentry::start_transaction(sentry::TransactionContext::new(name, op)).into(),
        };
        for (key, value) in attributes {
            span.set_data(key, value.clone());
        }
        sentry::configure_scope(|scope| scope.set_span(Some(span.clone())));
        Box::new(SentryTiming { span, parent })
    }

    fn warn(&self, message: &'static str, attributes: &[(&'static str, Value)]) {
        sentry::Hub::current().capture_log(sentry::protocol::Log {
            level: sentry::protocol::LogLevel::Warn,
            body: message.into(),
            trace_id: None,
            timestamp: std::time::SystemTime::now(),
            severity_number: None,
            attributes: attributes
                .iter()
                .map(|(key, value)| {
                    (
                        (*key).to_owned(),
                        sentry::protocol::LogAttribute(value.clone()),
                    )
                })
                .collect(),
        });
    }
}

struct SentryTiming {
    span: sentry::TransactionOrSpan,
    parent: Option<sentry::TransactionOrSpan>,
}

impl arrumo_workspaces::TimingEnd for SentryTiming {
    fn end(self: Box<Self>, ok: bool) {
        let SentryTiming { span, parent } = *self;
        span.set_status(if ok {
            sentry::protocol::SpanStatus::Ok
        } else {
            sentry::protocol::SpanStatus::InternalError
        });
        span.finish();
        sentry::configure_scope(|scope| scope.set_span(parent));
    }
}

/// Every report leaves through this transport. The frontend plugin forwards envelopes it cannot
/// parse, such as interface errors that carry source map debug IDs, straight to the transport, so
/// `before_send` never sees them: the setting is checked again here and those envelopes are
/// scrubbed the same way.
struct GuardedTransports;

impl sentry::TransportFactory for GuardedTransports {
    fn create_transport_with_options(
        &self,
        options: sentry::TransportOptions,
    ) -> Arc<dyn sentry::Transport> {
        Arc::new(GuardedTransport(
            sentry::transports::DefaultTransportFactory.create_transport_with_options(options),
        ))
    }
}

struct GuardedTransport(Arc<dyn sentry::Transport>);

impl sentry::Transport for GuardedTransport {
    fn send_envelope(&self, envelope: sentry::Envelope) {
        let allowed = Allowed {
            reports: arrumo_workspaces::crash_reports_allowed(),
            usage: arrumo_workspaces::usage_data_allowed(),
        };
        if let Some(envelope) = guard(envelope, allowed) {
            self.0.send_envelope(envelope);
        }
    }

    fn flush(&self, timeout: Duration) -> bool {
        self.0.flush(timeout)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.0.shutdown(timeout)
    }
}

#[derive(Clone, Copy)]
struct Allowed {
    /// Crash reports: errors and anything else that is not a timing or a log.
    reports: bool,
    /// Usage data: timings and logs.
    usage: bool,
}

fn guard(envelope: sentry::Envelope, allowed: Allowed) -> Option<sentry::Envelope> {
    use sentry::protocol::EnvelopeItem;
    // Parsed envelopes have been through `before_send`; unparsed ones have no items to list.
    if envelope.items().next().is_some() {
        let mut out = sentry::Envelope::new().with_headers(envelope.headers().clone());
        for item in envelope.into_items() {
            match item {
                EnvelopeItem::Transaction(mut transaction) if allowed.usage => {
                    transaction.user = None;
                    transaction.request = None;
                    transaction.server_name = None;
                    transaction.extra.clear();
                    out.add_item(EnvelopeItem::Transaction(transaction));
                }
                EnvelopeItem::ItemContainer(logs) if allowed.usage => out.add_item(logs),
                EnvelopeItem::Transaction(_) | EnvelopeItem::ItemContainer(_) => {}
                item if allowed.reports => out.add_item(item),
                _ => {}
            }
        }
        return out.items().next().is_some().then_some(out);
    }
    if !allowed.reports {
        return None;
    }
    let mut bytes = Vec::new();
    envelope.to_writer(&mut bytes).ok()?;
    let bytes = scrub_unparsed(&bytes)?;
    sentry::Envelope::from_bytes_raw(bytes).ok()
}

/// Rewrites an envelope's events with `scrub_json`, keeping only event items. Returns `None` for
/// anything that does not read as an envelope, so it is dropped rather than sent unscrubbed.
fn scrub_unparsed(bytes: &[u8]) -> Option<Vec<u8>> {
    let (header, mut rest) = split_line(bytes);
    serde_json::from_slice::<Value>(header).ok()?;
    let mut out = header.to_vec();
    out.push(b'\n');
    let mut events = 0;
    while !rest.is_empty() {
        let (item, after) = split_line(rest);
        if item.is_empty() {
            rest = after;
            continue;
        }
        let item: Value = serde_json::from_slice(item).ok()?;
        let (payload, after) = match item.get("length").and_then(Value::as_u64) {
            Some(length) => {
                let length = usize::try_from(length).ok()?;
                let payload = after.get(..length)?;
                let after = &after[length..];
                (payload, after.strip_prefix(b"\n").unwrap_or(after))
            }
            None => split_line(after),
        };
        rest = after;
        if item.get("type").and_then(Value::as_str) != Some("event") {
            continue;
        }
        let mut event: Value = serde_json::from_slice(payload).ok()?;
        scrub_json(&mut event);
        let payload = serde_json::to_vec(&event).ok()?;
        let header = serde_json::json!({ "type": "event", "length": payload.len() });
        out.extend(serde_json::to_vec(&header).ok()?);
        out.push(b'\n');
        out.extend(payload);
        out.push(b'\n');
        events += 1;
    }
    (events > 0).then_some(out)
}

fn split_line(bytes: &[u8]) -> (&[u8], &[u8]) {
    match bytes.iter().position(|byte| *byte == b'\n') {
        Some(end) => (&bytes[..end], &bytes[end + 1..]),
        None => (bytes, &[]),
    }
}

/// `scrub` for an event that is still JSON.
fn scrub_json(event: &mut Value) {
    let Some(event) = event.as_object_mut() else {
        return;
    };
    for field in ["user", "server_name", "breadcrumbs", "request"] {
        event.remove(field);
    }
    for field in ["message", "logentry"] {
        match event.get_mut(field) {
            Some(Value::String(text)) => *text = arrumo_workspaces::redact_report_text(text),
            Some(Value::Object(entry)) => {
                for key in ["message", "formatted"] {
                    if let Some(Value::String(text)) = entry.get_mut(key) {
                        *text = arrumo_workspaces::redact_report_text(text);
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(Value::Array(values)) = event
        .get_mut("exception")
        .and_then(|exception| exception.get_mut("values"))
    {
        for exception in values {
            if let Some(Value::String(text)) = exception.get_mut("value") {
                *text = arrumo_workspaces::redact_report_text(text);
            }
        }
    }
}

/// The crash report choice is also kept in a small file beside the data, so failures before the
/// data opens follow it too.
const CHOICE_FILE: &str = "crash-reports-on";

pub fn restore_choice(data_dir: &Path) {
    arrumo_workspaces::allow_crash_reports(data_dir.join(CHOICE_FILE).exists());
}

pub fn remember_choice(data_dir: &Path, allowed: bool) {
    let file = data_dir.join(CHOICE_FILE);
    let _ = if allowed {
        std::fs::write(file, b"")
    } else {
        std::fs::remove_file(file)
    };
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNPARSED: &[u8] = br#"{"event_id":"9ec79c33ec9942ab8353589fcb2e04dc"}
{"type":"event"}
{"event_id":"9ec79c33ec9942ab8353589fcb2e04dc","platform":"javascript","user":{"ip_address":"{{auto}}"},"server_name":"studio-mac","breadcrumbs":[{"message":"Opened My Holiday.mp4"}],"request":{"url":"tauri://localhost/videos?id=1"},"message":"Could not open /Users/ana/Movies/clip.mp4","exception":{"values":[{"type":"TypeError","value":"Failed at /Users/ana/a.js for ana@example.com"}]},"debug_meta":{"images":[{"type":"sourcemap","code_file":"app:///_next/static/chunks/a.js","debug_id":"011c4791-62f8-553d-b55b-d23bc9051a96"}]}}
{"type":"attachment","length":5,"filename":"notes.txt"}
hello
"#;

    fn events(bytes: &[u8]) -> Vec<Value> {
        let text = std::str::from_utf8(bytes).unwrap();
        let mut lines = text.lines().skip(1);
        let mut events = Vec::new();
        while let Some(header) = lines.next() {
            let header: Value = serde_json::from_str(header).unwrap();
            let payload = lines.next().unwrap();
            assert_eq!(header["length"].as_u64().unwrap() as usize, payload.len());
            assert_eq!(header["type"], "event");
            events.push(serde_json::from_str(payload).unwrap());
        }
        events
    }

    #[test]
    fn unparsed_interface_reports_are_scrubbed_and_keep_their_debug_ids() {
        assert!(sentry::Envelope::from_slice(UNPARSED).is_err());
        let envelope = sentry::Envelope::from_bytes_raw(UNPARSED.to_vec()).unwrap();
        let envelope = guard(envelope, ALL).unwrap();
        let mut bytes = Vec::new();
        envelope.to_writer(&mut bytes).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        for private in [
            "ana",
            "studio-mac",
            "Holiday",
            "localhost/videos",
            "notes.txt",
            "hello",
        ] {
            assert!(!text.contains(private), "{private} was sent: {text}");
        }
        let events = events(&bytes);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["message"], "Could not open [redacted]");
        assert_eq!(
            events[0]["exception"]["values"][0]["value"],
            "Failed at [redacted] for [redacted]"
        );
        assert_eq!(
            events[0]["debug_meta"]["images"][0]["debug_id"],
            "011c4791-62f8-553d-b55b-d23bc9051a96"
        );
    }

    const ALL: Allowed = Allowed {
        reports: true,
        usage: true,
    };

    fn mixed() -> sentry::Envelope {
        let mut envelope = sentry::Envelope::new();
        envelope.add_item(sentry::protocol::Event::default());
        envelope.add_item(sentry::protocol::Transaction {
            name: Some("script".into()),
            server_name: Some("studio-mac".into()),
            user: Some(sentry::User {
                username: Some("ana".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        envelope.add_item(sentry::protocol::EnvelopeItem::ItemContainer(
            vec![sentry::protocol::Log {
                level: sentry::protocol::LogLevel::Warn,
                body: "A generation step failed".into(),
                trace_id: None,
                timestamp: std::time::SystemTime::now(),
                severity_number: None,
                attributes: Default::default(),
            }]
            .into(),
        ));
        envelope
    }

    fn kinds(envelope: Option<sentry::Envelope>) -> Vec<&'static str> {
        use sentry::protocol::EnvelopeItem;
        envelope
            .map(|envelope| {
                envelope
                    .into_items()
                    .map(|item| match item {
                        EnvelopeItem::Event(_) => "event",
                        EnvelopeItem::Transaction(_) => "timing",
                        EnvelopeItem::ItemContainer(_) => "logs",
                        _ => "other",
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn each_setting_covers_its_own_items() {
        let reports_only = Allowed {
            reports: true,
            usage: false,
        };
        let usage_only = Allowed {
            reports: false,
            usage: true,
        };
        let none = Allowed {
            reports: false,
            usage: false,
        };
        assert_eq!(kinds(guard(mixed(), ALL)), ["event", "timing", "logs"]);
        assert_eq!(kinds(guard(mixed(), reports_only)), ["event"]);
        assert_eq!(kinds(guard(mixed(), usage_only)), ["timing", "logs"]);
        assert!(guard(mixed(), none).is_none());
        let unparsed = sentry::Envelope::from_bytes_raw(UNPARSED.to_vec()).unwrap();
        assert!(guard(unparsed, usage_only).is_none());
    }

    #[test]
    fn timings_leave_out_the_machine_and_user() {
        let envelope = guard(mixed(), ALL).unwrap();
        let mut bytes = Vec::new();
        envelope.to_writer(&mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("script"));
        assert!(
            !text.contains("studio-mac") && !text.contains("ana"),
            "{text}"
        );
    }

    #[derive(Default)]
    struct Sent(std::sync::Mutex<Vec<String>>);

    impl sentry::Transport for Sent {
        fn send_envelope(&self, envelope: sentry::Envelope) {
            let mut bytes = Vec::new();
            envelope.to_writer(&mut bytes).unwrap();
            self.0
                .lock()
                .unwrap()
                .push(String::from_utf8(bytes).unwrap());
        }
    }

    #[test]
    fn timings_and_warnings_follow_usage_data_and_leave_out_the_machine() {
        use arrumo_workspaces::Diagnostics;
        let store = arrumo_workspaces::WorkspaceStore::open(":memory:").unwrap();
        let sent = Arc::new(Sent::default());
        let run = |usage: bool| {
            store.set_privacy_settings(usage, false).unwrap();
            let mut options = options();
            options.dsn = Some("https://key@reports.invalid/1".parse().unwrap());
            options.server_name = Some("studio-mac".into());
            let inner: Arc<dyn sentry::Transport> = sent.clone();
            options.transport = Some(Arc::new(Arc::new(GuardedTransport(inner))));
            let client = Arc::new(sentry::Client::from(options));
            let hub = Arc::new(sentry::Hub::new(Some(client.clone()), Default::default()));
            sentry::Hub::run(hub, || {
                let step = SentryDiagnostics.start(
                    "generation.step",
                    "script",
                    &[("agent", "test-agent".into())],
                );
                SentryDiagnostics
                    .start("process", "arrumo-ffmpeg", &[])
                    .end(true);
                SentryDiagnostics.warn(
                    "A generation step failed at /Users/ana/Movies",
                    &[("stage", "script".into()), ("folder", "/Users/ana".into())],
                );
                step.end(false);
            });
            client.flush(Some(Duration::from_secs(5)));
            std::mem::take(&mut *sent.0.lock().unwrap()).join("\n")
        };
        let text = run(true);
        for expected in [
            "generation.step",
            "script",
            "test-agent",
            "arrumo-ffmpeg",
            "A generation step failed at [redacted]",
        ] {
            assert!(text.contains(expected), "{expected} missing: {text}");
        }
        for private in ["studio-mac", "/Users/ana", "folder"] {
            assert!(!text.contains(private), "{private} was sent: {text}");
        }
        assert_eq!(run(false), "");
    }

    #[test]
    fn unreadable_envelopes_are_dropped() {
        for bytes in [
            &b"not json\n{}\n"[..],
            b"{}\n{\"type\":\"event\",\"length\":99}\n{}\n",
            b"{}\n",
        ] {
            let envelope = sentry::Envelope::from_bytes_raw(bytes.to_vec()).unwrap();
            assert!(guard(envelope, ALL).is_none());
        }
    }
}
