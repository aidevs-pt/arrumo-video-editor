use std::path::PathBuf;

use arrumo_workspaces::{BackupSummary, WorkspaceStore};
use tauri::{AppHandle, Manager};

fn database(app: &AppHandle) -> Result<PathBuf, &'static str> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite"))
}

#[tauri::command]
pub async fn export_backup(
    app: AppHandle,
    destination: PathBuf,
) -> Result<BackupSummary, &'static str> {
    let database = database(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .export_backup(&database, &destination)
            .map_err(|_| "backup_export_failed")
    })
    .await
    .map_err(|_| "backup_export_failed")?
}

/// Checks and stages a backup to replace everything at the next start.
#[tauri::command]
pub async fn stage_backup_import(
    app: AppHandle,
    archive: PathBuf,
) -> Result<BackupSummary, String> {
    let database = database(&app).map_err(str::to_owned)?;
    tauri::async_runtime::spawn_blocking(move || {
        arrumo_workspaces::stage_backup_import(&database, &archive)
            .map_err(|error| import_error(&error.to_string()).to_owned())
    })
    .await
    .map_err(|_| "backup_import_failed".to_owned())?
}

#[tauri::command]
pub fn pending_backup_import(app: AppHandle) -> Result<bool, &'static str> {
    Ok(arrumo_workspaces::pending_backup_import(&database(&app)?))
}

#[tauri::command]
pub fn cancel_backup_import(app: AppHandle) -> Result<(), &'static str> {
    arrumo_workspaces::cancel_backup_import(&database(&app)?).map_err(|_| "backup_cancel_failed")
}

#[tauri::command]
pub fn restart_to_restore(app: AppHandle) {
    app.restart();
}

/// A code the interface translates, from the reason the backup was refused.
fn import_error(message: &str) -> &'static str {
    if message.contains("not an Arrumo backup") || message.contains("outside its folders") {
        "backup_not_arrumo"
    } else if message.contains("changed or damaged")
        || message.contains("do not match")
        || message.contains("missing")
    {
        "backup_damaged"
    } else if message.contains("newer version") || message.contains("unsupported version") {
        "backup_newer"
    } else {
        "backup_import_failed"
    }
}
