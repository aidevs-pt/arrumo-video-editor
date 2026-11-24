//! App updates from the signed GitHub releases. Builds without `ARRUMO_UPDATER_PUBKEY` (contributor
//! builds) or with `ARRUMO_STORE_BUILD` (stores update the app themselves) never check.
//!
//! A launch marker guards against a release that breaks on start. It is written before anything
//! else runs and removed once the interface has stayed up for a while. When a launch finds it
//! left behind, Arrumo checks for a newer release before opening any data, and installs it
//! straight away, so a release that fails during startup can still be replaced.

use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

const PUBKEY: Option<&str> = option_env!("ARRUMO_UPDATER_PUBKEY");
const STORE_BUILD: Option<&str> = option_env!("ARRUMO_STORE_BUILD");
/// How long a check and download may hold up a launch after a failed one.
const RECOVERY_LIMIT: Duration = Duration::from_secs(90);
/// The delay before the routine check, so it never slows the first screen.
const FIRST_CHECK: Duration = Duration::from_secs(15);

/// The update the creator was offered, and its download once they chose to install on quit.
#[derive(Default)]
pub struct Updates {
    offered: Mutex<Option<Update>>,
    on_quit: Mutex<Option<(Update, Vec<u8>)>>,
    /// Set once the interface has rendered, so quitting counts as a normal launch.
    shown: AtomicBool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// Whether this build updates itself; store and contributor builds do not.
    enabled: bool,
    version: String,
    store: bool,
}

/// The key that signs releases, and the Announcements list; `None` in contributor builds.
pub fn public_key() -> Option<&'static str> {
    PUBKEY.filter(|key| !key.trim().is_empty())
}

pub fn enabled() -> bool {
    PUBKEY.is_some_and(|key| !key.trim().is_empty()) && STORE_BUILD.is_none()
}

/// The updater plugin, for builds that update themselves.
pub fn plugin<R: Runtime>() -> Option<tauri::plugin::TauriPlugin<R, tauri_plugin_updater::Config>> {
    enabled().then(|| {
        tauri_plugin_updater::Builder::new()
            .pubkey(PUBKEY.unwrap_or_default())
            .build()
    })
}

fn marker(app: &AppHandle) -> Option<PathBuf> {
    Some(
        app.path()
            .app_local_data_dir()
            .ok()?
            .join("launch-in-progress"),
    )
}

/// Writes the launch marker and starts the update checks. Call first in setup: after a failed
/// launch this blocks while it looks for, and installs, a newer release.
pub fn start(app: &AppHandle) {
    let Some(marker) = marker(app) else {
        return;
    };
    let failed_before = marker.exists();
    let _ = std::fs::create_dir_all(marker.parent().unwrap_or(&marker));
    let _ = std::fs::write(&marker, b"");
    if !enabled() {
        return;
    }
    if failed_before {
        let installed = tauri::async_runtime::block_on(async {
            tokio::time::timeout(RECOVERY_LIMIT, async {
                let update = check(app).await.ok().flatten()?;
                update.download_and_install(|_, _| {}, || {}).await.ok()
            })
            .await
            .ok()
            .flatten()
            .is_some()
        });
        if installed {
            app.restart();
        }
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        if let Ok(Some(update)) = check(&app).await {
            let _ = app.emit("update-available", describe(&update));
            *app.state::<Updates>()
                .offered
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(update);
        }
    });
}

/// The interface reports it rendered (`settled` false), then that it stayed up (`settled` true),
/// which ends the launch's guard.
#[tauri::command]
pub fn launch_ready(settled: bool, app: AppHandle, updates: tauri::State<'_, Updates>) {
    updates.shown.store(true, Ordering::Relaxed);
    if settled && let Some(marker) = marker(&app) {
        let _ = std::fs::remove_file(marker);
    }
}

/// Clears the launch marker when the interface came up, and installs an update the creator chose to install on quit.
pub fn on_exit(app: &AppHandle) {
    let shown = app
        .try_state::<Updates>()
        .is_some_and(|updates| updates.shown.load(Ordering::Relaxed));
    if shown && let Some(marker) = marker(app) {
        let _ = std::fs::remove_file(marker);
    }
    let pending = app
        .try_state::<Updates>()
        .and_then(|updates| updates.on_quit.lock().ok()?.take());
    if let Some((update, bytes)) = pending {
        let _ = update.install(bytes);
    }
}

async fn check(app: &AppHandle) -> Result<Option<Update>, tauri_plugin_updater::Error> {
    app.updater()?.check().await
}

fn describe(update: &Update) -> AvailableUpdate {
    AvailableUpdate {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update
            .body
            .as_deref()
            .map(|notes| notes.chars().take(4000).collect()),
    }
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateStatus {
    UpdateStatus {
        enabled: enabled(),
        version: app.package_info().version.to_string(),
        store: STORE_BUILD.is_some(),
    }
}

/// Checks now, remembering the update so it can be installed.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<AvailableUpdate>, &'static str> {
    if !enabled() {
        return Ok(None);
    }
    let update = check(&app).await.map_err(|_| "update_check_failed")?;
    let described = update.as_ref().map(describe);
    *app.state::<Updates>()
        .offered
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = update;
    Ok(described)
}

/// Downloads the offered update, then restarts into it (`now`) or installs it when Arrumo quits.
#[tauri::command]
pub async fn install_update(now: bool, app: AppHandle) -> Result<(), &'static str> {
    let update = app
        .state::<Updates>()
        .offered
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or("update_not_offered")?;
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| "update_download_failed")?;
    if now {
        update.install(bytes).map_err(|_| "update_install_failed")?;
        app.restart();
    }
    *app.state::<Updates>()
        .on_quit
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
    Ok(())
}
