#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};

use arrumo_workspaces::{Profile, Workspace, WorkspaceStore};
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
            update_profile
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Arrumo");
}
