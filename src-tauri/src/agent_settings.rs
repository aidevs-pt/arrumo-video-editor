use arrumo_workspaces::{AgentPlugin, AgentSettings};
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

#[tauri::command]
pub fn agent_cli_detected(provider: String) -> bool {
    arrumo_workspaces::agent_cli_detected(&provider)
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
