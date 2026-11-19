use std::{collections::BTreeMap, fs::File};

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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryListing {
    registry: arrumo_workspaces::PluginRegistry,
    supported_manifest_schema_versions: [u32; 2],
    update_versions: BTreeMap<String, Vec<String>>,
}

#[tauri::command]
pub async fn browse_plugin_registry(
    app: AppHandle,
    catalog_url: String,
) -> Result<RegistryListing, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = arrumo_workspaces::fetch_plugin_registry(&catalog_url)
            .map_err(|_| "plugin_registry_failed")?;
        let installed = app
            .state::<Store>()
            .lock()
            .map_err(|_| "store_unavailable")?
            .plugin_catalog()
            .map_err(|_| "plugin_catalog_failed")?;
        let update_versions = installed
            .into_iter()
            .filter_map(|plugin| {
                let source = plugin.package?.release?;
                let versions = registry.update_versions(&catalog_url, &source);
                (!versions.is_empty()).then_some((source.plugin_id, versions))
            })
            .collect();
        Ok(RegistryListing {
            update_versions,
            registry,
            supported_manifest_schema_versions:
                arrumo_workspaces::SUPPORTED_PLUGIN_MANIFEST_SCHEMAS,
        })
    })
    .await
    .map_err(|_| "plugin_registry_failed")?
}

#[tauri::command]
pub async fn install_registry_plugin(
    app: AppHandle,
    catalog_url: String,
    id: String,
    repository: String,
    release: arrumo_workspaces::RegistryRelease,
    update: bool,
) -> Result<CatalogPlugin, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let store = WorkspaceStore::open(database).map_err(|_| "store_unavailable")?;
        if update {
            store.update_reviewed_plugin_release(&catalog_url, &id, &repository, &release)
        } else {
            store.download_reviewed_plugin_release(
                &catalog_url,
                &id,
                &release.version,
                &repository,
                &release,
            )
        }
        .map_err(|_| "plugin_registry_install_failed")
    })
    .await
    .map_err(|_| "plugin_registry_install_failed")?
}

#[tauri::command]
pub fn open_plugin_repository(app: AppHandle, url: String) -> Result<(), &'static str> {
    use tauri_plugin_opener::OpenerExt;

    arrumo_workspaces::validate_registry_url(&url).map_err(|_| "plugin_repository_failed")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "plugin_repository_failed")
}
