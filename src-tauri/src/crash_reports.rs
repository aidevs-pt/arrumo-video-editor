//! Crash and error reports through Sentry, for Rust panics and errors and for the web frontend.
//! Builds without `ARRUMO_CRASH_DSN` start nothing. Every report is dropped while the creator has
//! crash reports turned off, and reports carry no personal data. Native crash dumps are not
//! collected: they hold raw memory that could include the creator's content.

use std::path::Path;

use std::borrow::Cow;

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
