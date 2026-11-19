#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};

use arrumo_workspaces::{
    DeliveryOverrides, DimensionOverrides, FormatDelivery, OutputFormat, OutputFormatSettings,
    Profile, ProfileDelivery, ProfileFormat, SupplementalOutputs, Workspace, WorkspaceAsset,
    WorkspaceFeedback, WorkspaceStore, asset_root,
};
use tauri::{AppHandle, Manager, State};

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

#[tauri::command]
async fn list_profile_formats(
    workspace_id: i64,
    profile_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<ProfileFormat>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_profile_formats(workspace_id, profile_id)
        .map_err(|_| "profile_format_list_failed")
}

#[tauri::command]
async fn set_profile_format_overrides(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    overrides: DimensionOverrides,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_format_overrides(workspace_id, profile_id, format_id, &overrides)
        .map_err(|_| "profile_format_save_failed")
}

#[tauri::command]
async fn get_format_delivery(
    workspace_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<FormatDelivery, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .format_delivery(workspace_id, format_id)
        .map_err(|_| "format_delivery_load_failed")
}

#[tauri::command]
async fn set_format_delivery(
    workspace_id: i64,
    format_id: i64,
    settings: SupplementalOutputs,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_format_delivery(workspace_id, format_id, &settings)
        .map_err(|_| "format_delivery_save_failed")
}

#[tauri::command]
async fn get_profile_delivery(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<ProfileDelivery, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .profile_delivery(workspace_id, profile_id, format_id)
        .map_err(|_| "profile_delivery_load_failed")
}

#[tauri::command]
async fn set_profile_delivery(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    overrides: DeliveryOverrides,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_delivery(workspace_id, profile_id, format_id, &overrides)
        .map_err(|_| "profile_delivery_save_failed")
}

#[tauri::command]
async fn list_assets(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_assets(workspace_id)
        .map_err(|_| "asset_list_failed")
}

#[tauri::command]
async fn import_asset(
    workspace_id: i64,
    source: PathBuf,
    thumbnail_reference: Option<bool>,
    app: AppHandle,
    store: State<'_, Store>,
) -> Result<WorkspaceAsset, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "asset_storage_unavailable")?
        .join("workspaces.sqlite");
    let root = asset_root(&database);
    let mut store = store.lock().map_err(|_| "store_unavailable")?;
    let imported = if thumbnail_reference.unwrap_or(false) {
        store.import_thumbnail_reference(&root, workspace_id, &source)
    } else {
        store.import_asset(&root, workspace_id, &source)
    };
    imported.map_err(|_| "asset_import_failed")
}

#[tauri::command]
async fn set_asset_notes(
    workspace_id: i64,
    asset_id: i64,
    notes: String,
    store: State<'_, Store>,
) -> Result<WorkspaceAsset, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_asset_notes(workspace_id, asset_id, &notes)
        .map_err(|_| "asset_notes_failed")
}

#[tauri::command]
async fn get_workspace_context(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Option<String>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .workspace_context(workspace_id)
        .map_err(|_| "workspace_context_load_failed")
}

#[tauri::command]
async fn set_workspace_context(
    workspace_id: i64,
    markdown: String,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_workspace_context(workspace_id, &markdown)
        .map_err(|_| "workspace_context_save_failed")
}

#[tauri::command]
async fn list_workspace_feedback(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceFeedback>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_workspace_feedback(workspace_id)
        .map_err(|_| "feedback_load_failed")
}

#[tauri::command]
async fn update_workspace_feedback(
    workspace_id: i64,
    feedback_id: i64,
    title: String,
    markdown: String,
    store: State<'_, Store>,
) -> Result<WorkspaceFeedback, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_workspace_feedback(workspace_id, feedback_id, &title, &markdown)
        .map_err(|_| "feedback_save_failed")
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            remove_output_format,
            list_profile_formats,
            set_profile_format_overrides,
            get_format_delivery,
            set_format_delivery,
            get_profile_delivery,
            set_profile_delivery,
            list_assets,
            import_asset,
            set_asset_notes,
            get_workspace_context,
            set_workspace_context,
            list_workspace_feedback,
            update_workspace_feedback
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Arrumo");
}
