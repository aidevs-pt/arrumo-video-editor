#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};

use arrumo_workspaces::{Workspace, WorkspaceStore};
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
        .invoke_handler(tauri::generate_handler![list_workspaces, create_workspace])
        .run(tauri::generate_context!())
        .expect("failed to start Arrumo");
}
