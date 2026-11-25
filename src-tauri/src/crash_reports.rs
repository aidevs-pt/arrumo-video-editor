//! Crash and error reports through Sentry, for Rust panics and errors and for the web frontend.
//! Builds without `ARRUMO_CRASH_DSN` start nothing. Every report is dropped while the creator has
//! crash reports turned off, and reports carry no personal data. Native crash dumps are not
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
    let mut options = sentry::ClientOptions::new();
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
    options.transport = Some(Arc::new(GuardedTransports));
    let options = options.add_integration(plugin.clone());
    let guard = sentry::init((dsn, options));
    (CrashReports(Some(guard)), Some(plugin))
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
        if let Some(envelope) = guard(envelope, arrumo_workspaces::crash_reports_allowed()) {
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

fn guard(envelope: sentry::Envelope, allowed: bool) -> Option<sentry::Envelope> {
    if !allowed {
        return None;
    }
    // Parsed envelopes have been through `before_send`; unparsed ones have no items to list.
    if envelope.items().next().is_some() {
        return Some(envelope);
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
        let envelope = guard(envelope, true).unwrap();
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

    #[test]
    fn nothing_is_sent_while_crash_reports_are_off() {
        let unparsed = sentry::Envelope::from_bytes_raw(UNPARSED.to_vec()).unwrap();
        assert!(guard(unparsed, false).is_none());
        let mut parsed = sentry::Envelope::new();
        parsed.add_item(sentry::protocol::Event::default());
        assert!(guard(parsed, false).is_none());
    }

    #[test]
    fn unreadable_envelopes_are_dropped() {
        for bytes in [
            &b"not json\n{}\n"[..],
            b"{}\n{\"type\":\"event\",\"length\":99}\n{}\n",
            b"{}\n",
        ] {
            let envelope = sentry::Envelope::from_bytes_raw(bytes.to_vec()).unwrap();
            assert!(guard(envelope, true).is_none());
        }
    }
}
