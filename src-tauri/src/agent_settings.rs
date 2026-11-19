use arrumo_workspaces::{AgentProvider, AgentSettings};
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
    provider: Option<AgentProvider>,
    store: State<'_, Store>,
) -> Result<AgentSettings, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    store
        .set_agent_provider(workspace_id, provider)
        .map_err(|_| "agent_settings_save_failed")?;
    store
        .agent_settings(workspace_id)
        .map_err(|_| "agent_settings_failed")
}

#[tauri::command]
pub fn agent_cli_detected(provider: AgentProvider) -> bool {
    arrumo_workspaces::agent_cli_detected(provider)
}

#[tauri::command]
pub async fn agent_login_status(
    provider: AgentProvider,
) -> Result<arrumo_workspaces::AgentLoginStatus, &'static str> {
    tauri::async_runtime::spawn_blocking(move || arrumo_workspaces::check_agent_login(provider))
        .await
        .map_err(|_| "agent_login_check_failed")
}

#[tauri::command]
pub fn open_agent_setup_guide(
    provider: AgentProvider,
    app: tauri::AppHandle,
) -> Result<(), &'static str> {
    use tauri_plugin_opener::OpenerExt;

    let url = match provider {
        AgentProvider::Codex => "https://learn.chatgpt.com/docs/codex/cli",
        AgentProvider::ClaudeCode => "https://code.claude.com/docs/en/setup",
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "agent_setup_guide_failed")
}
