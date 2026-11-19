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
