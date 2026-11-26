use std::path::Path;

use arrumo_workspaces::{AgentExecutableError, AgentPlugin, AgentSettings};
use serde::Serialize;
use tauri::State;

use crate::Store;

#[tauri::command]
pub fn get_agent_settings(
    workspace_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<AgentSettings, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .agent_settings(workspace_id)
        .map_err(|_| "agent_settings_failed")
}

#[tauri::command]
pub fn set_agent_provider(
    workspace_id: Option<i64>,
    provider: Option<String>,
    store: State<'_, Store>,
) -> Result<AgentSettings, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    store
        .set_agent_provider(workspace_id, provider.as_deref())
        .map_err(|_| "agent_settings_save_failed")?;
    store
        .agent_settings(workspace_id)
        .map_err(|_| "agent_settings_failed")
}

/// Reads the PATH again, so an agent's CLI installed while Arrumo is open can be found.
#[tauri::command]
pub async fn refresh_agent_search_path() -> bool {
    tauri::async_runtime::spawn_blocking(|| {
        let current = arrumo_workspaces::agent_search_path().unwrap_or_default();
        #[cfg(unix)]
        let fresh = crate::shell_path::shell_path(current);
        #[cfg(windows)]
        let fresh = crate::windows_path::registry_path(current);
        #[cfg(not(any(unix, windows)))]
        let fresh: Option<std::ffi::OsString> = None;
        fresh
            .map(arrumo_workspaces::set_agent_search_path)
            .is_some()
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
pub fn agent_cli_detected(provider: String) -> bool {
    arrumo_workspaces::agent_cli_detected(&provider)
}

/// Where an agent's CLI was found, and the program chosen for it in settings, if any.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCliLocation {
    found: Option<String>,
    chosen: Option<String>,
}

fn location(provider: &str, store: &arrumo_workspaces::WorkspaceStore) -> AgentCliLocation {
    AgentCliLocation {
        found: arrumo_workspaces::agent_cli_path(provider)
            .map(|path| path.to_string_lossy().into_owned()),
        chosen: store.agent_executable(provider).ok().flatten(),
    }
}

#[tauri::command]
pub fn agent_cli_location(
    provider: String,
    store: State<'_, Store>,
) -> Result<AgentCliLocation, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    Ok(location(&provider, &store))
}

/// Chooses the program that runs an agent, or with no path goes back to finding it on the PATH.
/// Only the desktop offers this; the MCP and A2A servers agents talk to don't.
#[tauri::command]
pub fn set_agent_cli_path(
    provider: String,
    path: Option<String>,
    store: State<'_, Store>,
) -> Result<AgentCliLocation, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    store
        .set_agent_executable(&provider, path.as_deref().map(Path::new))
        .map_err(|error| match error {
            AgentExecutableError::NotRunnable => "agent_cli_not_runnable",
            AgentExecutableError::UnknownAgent => "agent_unknown",
            AgentExecutableError::Database(_) => "agent_cli_save_failed",
        })?;
    Ok(location(&provider, &store))
}

#[tauri::command]
pub async fn agent_login_status(
    provider: String,
) -> Result<arrumo_workspaces::AgentLoginStatus, &'static str> {
    tauri::async_runtime::spawn_blocking(move || arrumo_workspaces::check_agent_login(&provider))
        .await
        .map_err(|_| "agent_login_check_failed")
}

#[tauri::command]
pub fn open_agent_setup_guide(provider: String, app: tauri::AppHandle) -> Result<(), &'static str> {
    use tauri_plugin_opener::OpenerExt;

    let plugin = arrumo_workspaces::agent_plugin(&provider).ok_or("agent_plugin_unavailable")?;
    app.opener()
        .open_url(&plugin.setup_url, None::<&str>)
        .map_err(|_| "agent_setup_guide_failed")
}

#[tauri::command]
pub fn list_agent_plugins(
    store: State<'_, Store>,
) -> Result<Vec<&'static AgentPlugin>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .active_agent_plugins()
        .map_err(|_| "agent_plugins_failed")
}
