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
    use std::env;

    let executable = match provider {
        AgentProvider::Codex => "codex",
        AgentProvider::ClaudeCode => "claude",
    };
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .any(|directory| {
            #[cfg(windows)]
            {
                ["exe", "cmd", "bat"].iter().any(|extension| {
                    directory
                        .join(executable)
                        .with_extension(extension)
                        .is_file()
                })
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                directory.join(executable).metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            }
        })
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
