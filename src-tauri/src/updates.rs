//! App updates from the signed GitHub releases. Builds without `ARRUMO_UPDATER_PUBKEY` (contributor
//! builds) or with `ARRUMO_STORE_BUILD` (stores update the app themselves) never check.
//!
//! A launch marker guards against a release that crashes: it is written when Arrumo starts and
//! removed once the app has run for a minute or quit normally. When the previous launch left it
//! behind, Arrumo checks first and installs any newer release straight away, before the rest of
//! the app can fail again.

use std::{path::PathBuf, sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

const PUBKEY: Option<&str> = option_env!("ARRUMO_UPDATER_PUBKEY");
const STORE_BUILD: Option<&str> = option_env!("ARRUMO_STORE_BUILD");
/// How long Arrumo must run before a launch counts as healthy.
const HEALTHY_AFTER: Duration = Duration::from_secs(60);
/// The delay before the routine check, so it never slows the first screen.
const FIRST_CHECK: Duration = Duration::from_secs(15);

/// The update the creator was offered, and its download once they chose to install on quit.
#[derive(Default)]
pub struct Updates {
    offered: Mutex<Option<Update>>,
    on_quit: Mutex<Option<(Update, Vec<u8>)>>,
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

/// Starts the launch marker and the update checks. Call first in setup.
pub fn start(app: &AppHandle) {
    let Some(marker) = marker(app) else {
        return;
    };
    let crashed = marker.exists();
    let _ = std::fs::create_dir_all(marker.parent().unwrap_or(&marker));
    let _ = std::fs::write(&marker, b"");
    let healthy = marker.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(HEALTHY_AFTER).await;
        let _ = std::fs::remove_file(healthy);
    });
    if !enabled() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if crashed {
            // The last launch never got going: a newer release may be the fix.
            if let Ok(Some(update)) = check(&app).await
                && update.download_and_install(|_, _| {}, || {}).await.is_ok()
            {
                app.restart();
            }
            return;
        }
        tokio_sleep(FIRST_CHECK).await;
        if let Ok(Some(update)) = check(&app).await {
            let _ = app.emit("update-available", describe(&update));
            *app.state::<Updates>()
                .offered
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(update);
        }
    });
}

/// Clears the launch marker and installs an update the creator chose to install on quit.
pub fn on_exit(app: &AppHandle) {
    if let Some(marker) = marker(app) {
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

async fn tokio_sleep(duration: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
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
