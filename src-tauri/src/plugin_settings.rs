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
    supported_manifest_schema_versions: &'static [u32],
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
                &arrumo_workspaces::SUPPORTED_PLUGIN_MANIFEST_SCHEMAS,
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditablePluginSetting {
    app: Option<String>,
    workspace_override: Option<String>,
    effective: Option<String>,
    source: Option<arrumo_workspaces::PluginSettingScope>,
}

#[tauri::command]
pub fn read_plugin_settings(
    plugin_id: String,
    workspace_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<BTreeMap<String, EditablePluginSetting>, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    let settings = store
        .plugin_settings(&plugin_id, workspace_id)
        .map_err(|_| "plugin_settings_load_failed")?;
    Ok(settings
        .into_iter()
        .map(|(key, state)| {
            let encode = |value: serde_json::Value| value.to_string();
            (
                key,
                EditablePluginSetting {
                    app: state.app.map(encode),
                    workspace_override: state.workspace_override.map(encode),
                    effective: state.effective.map(encode),
                    source: state.source,
                },
            )
        })
        .collect())
}

#[tauri::command]
pub fn write_plugin_setting(
    plugin_id: String,
    setting_id: String,
    workspace_id: Option<i64>,
    value_json: Option<String>,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    // Text transport preserves the full range of signed integer settings in the webview.
    let value = value_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "plugin_setting_invalid")?;
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_plugin_setting(&plugin_id, &setting_id, workspace_id, value.as_ref())
        .map_err(|_| "plugin_setting_save_failed")
}

#[tauri::command]
pub async fn read_plugin_credential_status(
    app: AppHandle,
    plugin_id: String,
    setting_id: String,
    workspace_id: Option<i64>,
) -> Result<arrumo_workspaces::PluginCredentialStatus, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = open_credential_workspace_store(&app)?;
        store
            .plugin_credential_status(&plugin_id, &setting_id, workspace_id)
            .map_err(|_| "plugin_credential_status_failed")
    })
    .await
    .map_err(|_| "plugin_credential_status_failed")?
}

#[tauri::command]
pub async fn write_plugin_credential(
    app: AppHandle,
    plugin_id: String,
    setting_id: String,
    workspace_id: Option<i64>,
    secret: Option<String>,
) -> Result<arrumo_workspaces::PluginCredentialStatus, &'static str> {
    let secret = secret.map(|value| zeroize::Zeroizing::new(value.into_bytes()));
    tauri::async_runtime::spawn_blocking(move || {
        let store = open_credential_workspace_store(&app)?;
        store
            .set_plugin_credential(&plugin_id, &setting_id, workspace_id, secret)
            .map_err(|_| "plugin_credential_save_failed")?;
        store
            .plugin_credential_status(&plugin_id, &setting_id, workspace_id)
            .map_err(|_| "plugin_credential_status_failed")
    })
    .await
    .map_err(|_| "plugin_credential_save_failed")?
}

fn open_credential_workspace_store(app: &AppHandle) -> Result<WorkspaceStore, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    WorkspaceStore::open(database).map_err(|_| "store_unavailable")
}

#[tauri::command]
pub fn list_provider_preferences(
    workspace_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<BTreeMap<String, arrumo_workspaces::ProviderPreferences>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_provider_preferences(workspace_id)
        .map_err(|_| "provider_preferences_failed")
}

#[tauri::command]
pub fn list_plugin_surfaces(
    placement: arrumo_workspaces::PluginSurfacePlacement,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::InstalledPluginSurface>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_surfaces(placement)
        .map_err(|_| "plugin_surface_list_failed")
}

pub fn serve_plugin_surface(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Method, Response, StatusCode};

    const POLICY: &str = "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; base-uri 'none'; form-action 'none'; frame-src 'none'";
    let document = (|| {
        if request.method() != Method::GET
            || request.uri().query().is_some()
            || context.webview_label() != "main"
        {
            return None;
        }
        let mut parts = request.uri().path().strip_prefix('/')?.split('.');
        let (plugin_id, surface_id, sha256) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || sha256.len() != 64 {
            return None;
        }
        let store = context.app_handle().try_state::<Store>()?;
        store
            .lock()
            .ok()?
            .plugin_surface_document(plugin_id, surface_id, sha256)
            .ok()
    })();
    let (status, content_type, body) = match document {
        Some(document) => (
            StatusCode::OK,
            "text/html; charset=utf-8",
            document.into_bytes(),
        ),
        None => (
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            Vec::new(),
        ),
    };
    Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Content-Security-Policy", POLICY)
        .header("Cache-Control", "no-store")
        .header("Referrer-Policy", "no-referrer")
        .header("X-Content-Type-Options", "nosniff")
        .header("X-DNS-Prefetch-Control", "off")
        .body(body)
        .expect("valid fixed plugin response headers")
}
