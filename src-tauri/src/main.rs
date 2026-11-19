#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};

use arrumo_workspaces::{OutputFormat, OutputFormatSettings, Profile, Workspace, WorkspaceStore};
use tauri::{Manager, State};

type Store = Mutex<WorkspaceStore>;

#[tauri::command]
fn list_workspaces(store: State<'_, Store>) -> Result<Vec<Workspace>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list()
        .map_err(|_| "list_failed")
}

#[tauri::command]
fn create_workspace(name: String, store: State<'_, Store>) -> Result<Workspace, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create(&name)
        .map_err(|_| "create_failed")
}

#[tauri::command]
async fn list_profiles(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<Profile>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_profiles(workspace_id)
        .map_err(|_| "profiles_list_failed")
}

#[tauri::command]
async fn create_profile(
    workspace_id: i64,
    name: String,
    guidance: String,
    color: Option<String>,
    store: State<'_, Store>,
) -> Result<Profile, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_profile(workspace_id, &name, &guidance, color.as_deref())
        .map_err(|_| "profile_create_failed")
}

#[tauri::command]
async fn update_profile(
    workspace_id: i64,
    profile_id: i64,
    name: String,
    guidance: String,
    color: Option<String>,
    store: State<'_, Store>,
) -> Result<Profile, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_profile(workspace_id, profile_id, &name, &guidance, color.as_deref())
        .map_err(|_| "profile_update_failed")
}

#[tauri::command]
async fn list_output_formats(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<OutputFormat>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_output_formats(workspace_id)
        .map_err(|_| "format_list_failed")
}

#[tauri::command]
async fn create_output_format(
    workspace_id: i64,
    settings: OutputFormatSettings,
    store: State<'_, Store>,
) -> Result<OutputFormat, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_output_format(workspace_id, &settings)
        .map_err(|_| "format_create_failed")
}

#[tauri::command]
async fn update_output_format(
    workspace_id: i64,
    format_id: i64,
    settings: OutputFormatSettings,
    store: State<'_, Store>,
) -> Result<OutputFormat, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_output_format(workspace_id, format_id, &settings)
        .map_err(|_| "format_update_failed")
}

#[tauri::command]
async fn remove_output_format(
    workspace_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .remove_output_format(workspace_id, format_id)
        .map_err(|_| "format_remove_failed")
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir: PathBuf = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Mutex::new(WorkspaceStore::open(
                data_dir.join("workspaces.sqlite"),
            )?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            create_workspace,
            list_profiles,
            create_profile,
            update_profile,
            list_output_formats,
            create_output_format,
            update_output_format,
            remove_output_format
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Arrumo");
}
