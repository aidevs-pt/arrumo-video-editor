use std::fs::File;

use arrumo_workspaces::{CatalogPlugin, WorkspaceStore};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::Store;

#[tauri::command]
pub fn list_plugin_catalog(store: State<'_, Store>) -> Result<Vec<CatalogPlugin>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_catalog()
        .map_err(|_| "plugin_catalog_failed")
}

#[tauri::command]
pub async fn import_plugin_package(
    app: AppHandle,
    picker_title: String,
) -> Result<Option<CatalogPlugin>, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(source) = app
            .dialog()
            .file()
            .set_title(picker_title)
            .add_filter("ZIP", &["zip"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let source = source.into_path().map_err(|_| "plugin_import_failed")?;
        let file = File::open(source).map_err(|_| "plugin_import_failed")?;
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(database)
            .map_err(|_| "store_unavailable")?
            .install_plugin_zip(file)
            .map(Some)
            .map_err(|_| "plugin_import_failed")
    })
    .await
    .map_err(|_| "plugin_import_failed")?
}

#[tauri::command]
pub fn remove_plugin_package(id: String, store: State<'_, Store>) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .remove_installed_plugin_manifest(&id)
        .map_err(|_| "plugin_remove_failed")
}
