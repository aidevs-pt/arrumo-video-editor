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

/// Turns a plugin on or off for the whole app. Errors are codes the interface translates.
#[tauri::command]
pub fn set_plugin_active(
    plugin_id: String,
    active: bool,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_plugin_active(&plugin_id, active)
        .map_err(|error| {
            arrumo_workspaces::plugin_activation_refusal_code(error.as_ref())
                .unwrap_or("plugin_activation_failed")
        })
}

#[tauri::command]
pub fn pending_plugin_tour(
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::TourStop>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .pending_plugin_tour()
        .map_err(|_| "plugin_tour_failed")
}

#[tauri::command]
pub fn plugin_tour(
    ids: Vec<String>,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::TourStop>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_tour(&ids)
        .map_err(|_| "plugin_tour_failed")
}

#[tauri::command]
pub fn finish_plugin_tour(ids: Vec<String>, store: State<'_, Store>) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .mark_plugins_introduced(&ids)
        .map_err(|_| "plugin_tour_failed")
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
pub async fn install_plugin_repository(
    app: AppHandle,
    repository: String,
) -> Result<CatalogPlugin, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(database)
            .map_err(|_| "store_unavailable".to_owned())?
            .install_plugin_from_repository(&repository, None)
            .map_err(|error| match error.downcast_ref::<std::io::Error>() {
                Some(error) if error.kind() == std::io::ErrorKind::InvalidData => error.to_string(),
                _ => "The plugin couldn't be installed. Check the address and that the package is a valid Arrumo plugin.".to_owned(),
            })
    })
    .await
    .map_err(|_| "plugin_import_failed".to_owned())?
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

/// Opens the help page an installed plugin declares for one of its settings. The address comes
/// from the manifest, never from the caller.
#[tauri::command]
pub fn open_plugin_setting_help(
    app: AppHandle,
    store: State<'_, Store>,
    plugin_id: String,
    setting_id: String,
) -> Result<(), &'static str> {
    use tauri_plugin_opener::OpenerExt;

    let url = store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_catalog()
        .map_err(|_| "plugin_catalog_failed")?
        .into_iter()
        .find(|plugin| plugin.manifest.id == plugin_id)
        .and_then(|plugin| {
            plugin
                .manifest
                .setting_help_url(&setting_id)
                .map(str::to_owned)
        })
        .ok_or("plugin_setting_help_missing")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "plugin_setting_help_failed")
}

/// Opens a web link from app content, such as Markdown, in the system browser.
#[tauri::command]
pub fn open_external_link(app: AppHandle, url: String) -> Result<(), &'static str> {
    use tauri_plugin_opener::OpenerExt;

    let parsed = tauri::Url::parse(&url).map_err(|_| "external_link_invalid")?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || url.len() > 4096
    {
        return Err("external_link_invalid");
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|_| "external_link_failed")
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
        let store = open_app_workspace_store(&app)?;
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
        let store = open_app_workspace_store(&app)?;
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

fn open_app_workspace_store(app: &AppHandle) -> Result<WorkspaceStore, &'static str> {
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

/// Saves the order providers are tried in for one capability; `None` returns to the inherited order.
#[tauri::command]
pub fn write_provider_preferences(
    capability: String,
    workspace_id: Option<i64>,
    order: Option<Vec<String>>,
    store: State<'_, Store>,
) -> Result<BTreeMap<String, arrumo_workspaces::ProviderPreferences>, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    store
        .set_provider_preferences(&capability, workspace_id, order.as_deref())
        .map_err(|_| "provider_preferences_invalid")?;
    store
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

#[tauri::command]
pub fn list_plugin_context_requests(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::PluginContextRequest>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_context_requests(workspace_id)
        .map_err(|_| "plugin_grants_load_failed")
}

#[tauri::command]
pub fn apply_plugin_context_grant(
    plugin_id: String,
    workspace_id: i64,
    package_sha256: String,
    allowed: bool,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .apply_reviewed_plugin_context_grant(&plugin_id, workspace_id, &package_sha256, allowed)
        .map_err(|_| "plugin_grant_write_failed")
}

#[tauri::command]
pub fn read_plugin_surface_context(
    plugin_id: String,
    surface_id: String,
    package_sha256: String,
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Option<String>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_surface_context(&plugin_id, &surface_id, &package_sha256, workspace_id)
        .map_err(|_| "plugin_surface_context_unavailable")
}

#[tauri::command]
pub fn list_plugin_actions(
    workspace_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::InstalledPluginAction>, &'static str> {
    let actions = store
        .lock()
        .map_err(|_| "store_unavailable")?
        .plugin_actions()
        .map_err(|_| "plugin_action_list_failed")?;
    let scope = if workspace_id.is_some() {
        arrumo_workspaces::PluginActionScope::Workspace
    } else {
        arrumo_workspaces::PluginActionScope::App
    };
    Ok(actions
        .into_iter()
        .filter(|entry| entry.action.scope == scope)
        .collect())
}

#[tauri::command]
pub async fn invoke_reviewed_plugin_action(
    app: AppHandle,
    plugin_id: String,
    action_id: String,
    package_sha256: String,
    workspace_id: Option<i64>,
    input_json: String,
) -> Result<String, &'static str> {
    if input_json.len() > arrumo_workspaces::MAX_PLUGIN_IO_BYTES {
        return Err("plugin_action_input_invalid");
    }
    let input = serde_json::from_str(&input_json).map_err(|_| "plugin_action_input_invalid")?;
    tauri::async_runtime::spawn_blocking(move || {
        let output = open_app_workspace_store(&app)?
            .invoke_reviewed_plugin_action(
                &plugin_id,
                &action_id,
                &package_sha256,
                workspace_id,
                &input,
            )
            .map_err(|_| "plugin_action_failed")?;
        serde_json::to_string_pretty(&output).map_err(|_| "plugin_action_failed")
    })
    .await
    .map_err(|_| "plugin_action_failed")?
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopPluginActionRequest {
    id: String,
    workspace_id: Option<i64>,
    plugin_id: String,
    action_id: String,
    package_sha256: String,
    input_json: String,
}

#[tauri::command]
pub async fn queue_plugin_action_job(
    app: AppHandle,
    request: DesktopPluginActionRequest,
) -> Result<(), &'static str> {
    if request.input_json.len() > arrumo_workspaces::MAX_PLUGIN_IO_BYTES {
        return Err("plugin_action_input_invalid");
    }
    let request = arrumo_workspaces::PluginActionJobRequest {
        id: request.id,
        workspace_id: request.workspace_id,
        plugin_id: request.plugin_id,
        action_id: request.action_id,
        package_sha256: request.package_sha256,
        input: serde_json::from_str(&request.input_json)
            .map_err(|_| "plugin_action_input_invalid")?,
    };
    tauri::async_runtime::spawn_blocking(move || {
        open_app_workspace_store(&app)?
            .queue_plugin_action_job(&request)
            .map(|_| ())
            .map_err(|_| "plugin_action_queue_failed")
    })
    .await
    .map_err(|_| "plugin_action_queue_failed")?
}

#[tauri::command]
pub async fn list_plugin_action_jobs(
    app: AppHandle,
    workspace_id: Option<i64>,
) -> Result<Vec<serde_json::Value>, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        open_app_workspace_store(&app)?
            .list_plugin_action_jobs(workspace_id)
            .map_err(|_| "plugin_action_job_list_failed")?
            .into_iter()
            .map(desktop_job)
            .collect()
    })
    .await
    .map_err(|_| "plugin_action_job_list_failed")?
}

#[tauri::command]
pub async fn process_plugin_action_job(
    app: AppHandle,
    workspace_id: Option<i64>,
    id: String,
) -> Result<serde_json::Value, &'static str> {
    let id = id.parse().map_err(|_| "plugin_action_job_invalid")?;
    tauri::async_runtime::spawn_blocking(move || {
        open_app_workspace_store(&app)?
            .process_plugin_action_job(workspace_id, id)
            .map_err(|_| "plugin_action_job_failed")
            .and_then(desktop_job)
    })
    .await
    .map_err(|_| "plugin_action_job_failed")?
}

fn desktop_job(job: arrumo_workspaces::PluginActionJob) -> Result<serde_json::Value, &'static str> {
    let mut value = serde_json::to_value(job).map_err(|_| "plugin_action_job_failed")?;
    if matches!(value["state"].as_str(), Some("completed" | "failed")) {
        let serialized = serde_json::to_string_pretty(&value["result"])
            .map_err(|_| "plugin_action_job_failed")?;
        value["result"] = serde_json::Value::String(serialized);
    }
    Ok(value)
}

#[tauri::command]
pub fn get_canvas_renderer(
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::CanvasStatus, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .canvas_status()
        .map_err(|_| "canvas_status_failed")
}

/// Downloads or removes the canvas browser off the shared store, since a download takes minutes.
async fn change_canvas_renderer(
    app: AppHandle,
    install: bool,
) -> Result<arrumo_workspaces::CanvasStatus, &'static str> {
    let failed = if install {
        "canvas_install_failed"
    } else {
        "canvas_remove_failed"
    };
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| failed)?
        .join("workspaces.sqlite");
    tauri::async_runtime::spawn_blocking(move || {
        let store = WorkspaceStore::open(&database).map_err(|_| failed)?;
        if install {
            store.install_canvas_browser()
        } else {
            store.remove_canvas_browser()
        }
        .map_err(|_| failed)
    })
    .await
    .map_err(|_| failed)?
}

#[tauri::command]
pub async fn install_canvas_renderer(
    app: AppHandle,
) -> Result<arrumo_workspaces::CanvasStatus, &'static str> {
    change_canvas_renderer(app, true).await
}

#[tauri::command]
pub async fn remove_canvas_renderer(
    app: AppHandle,
) -> Result<arrumo_workspaces::CanvasStatus, &'static str> {
    change_canvas_renderer(app, false).await
}
