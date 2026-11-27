use arrumo_workspaces::{MotionLibraryItem, MotionLibrarySettings};
use tauri::State;

use crate::Store;

#[tauri::command]
pub async fn list_motion_library(
    workspace_id: i64,
    profile_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<Vec<MotionLibraryItem>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_motion_library(workspace_id, profile_id)
        .map_err(|_| "motion_library_load_failed")
}

#[tauri::command]
pub async fn create_motion_library_item(
    workspace_id: i64,
    settings: MotionLibrarySettings,
    store: State<'_, Store>,
) -> Result<MotionLibraryItem, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_motion_library_item(workspace_id, &settings)
        .map_err(|_| "motion_library_save_failed")
}

#[tauri::command]
pub async fn delete_motion_library_item(
    workspace_id: i64,
    item_id: i64,
    confirmed_title: String,
    app: tauri::AppHandle,
) -> Result<bool, &'static str> {
    let (database, store) = open_store(&app)?;
    store
        .delete_motion_library_item(&database, workspace_id, item_id, &confirmed_title)
        .map_err(|_| "motion_library_delete_failed")
}

#[tauri::command]
pub async fn update_motion_library_item(
    workspace_id: i64,
    item_id: i64,
    settings: MotionLibrarySettings,
    store: State<'_, Store>,
) -> Result<MotionLibraryItem, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_motion_library_item(workspace_id, item_id, &settings)
        .map_err(|_| "motion_library_save_failed")
}

#[tauri::command]
pub async fn duplicate_motion_library_item(
    workspace_id: i64,
    item_id: i64,
    title: String,
    store: State<'_, Store>,
) -> Result<MotionLibraryItem, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .duplicate_motion_library_item(workspace_id, item_id, &title)
        .map_err(|_| "motion_library_save_failed")
}

#[tauri::command]
pub async fn get_motion_library_shares(
    workspace_id: i64,
    item_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<i64>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .motion_library_shares(workspace_id, item_id)
        .map_err(|_| "motion_library_shares_load_failed")
}

#[tauri::command]
pub async fn set_motion_library_shares(
    workspace_id: i64,
    item_id: i64,
    profile_ids: Vec<i64>,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_motion_library_shares(workspace_id, item_id, &profile_ids)
        .map_err(|_| "motion_library_shares_save_failed")
}

#[derive(serde::Serialize)]
pub struct MotionPreview {
    video_url: String,
    sound_url: Option<String>,
}

fn motion_preview(
    database: &std::path::Path,
    store: &arrumo_workspaces::WorkspaceStore,
    workspace_id: i64,
    settings: &MotionLibrarySettings,
    stream: &crate::delivery_stream::DeliveryStream,
) -> Result<MotionPreview, &'static str> {
    let failed = "motion_library_preview_failed";
    let root = arrumo_workspaces::asset_root(database);
    let media_url = |id, kind| {
        let name = store
            .managed_asset_name(workspace_id, id, kind)
            .map_err(|_| failed)?;
        let mime = crate::delivery_stream::managed_media_type(&name).ok_or(failed)?;
        stream
            .open_media(arrumo_workspaces::asset_path(&root, workspace_id, id), mime)
            .map_err(|_| failed)
    };
    Ok(MotionPreview {
        video_url: media_url(settings.preview_asset_id, "video")?,
        sound_url: settings
            .sound_asset_id
            .map(|id| media_url(id, "audio"))
            .transpose()?,
    })
}

fn open_store(
    app: &tauri::AppHandle,
) -> Result<(std::path::PathBuf, arrumo_workspaces::WorkspaceStore), &'static str> {
    use tauri::Manager;

    let failed = "motion_library_preview_failed";
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| failed)?
        .join("workspaces.sqlite")
        .canonicalize()
        .map_err(|_| failed)?;
    let store = arrumo_workspaces::WorkspaceStore::open(&database).map_err(|_| failed)?;
    Ok((database, store))
}

#[tauri::command]
pub async fn preview_motion_library_item(
    workspace_id: i64,
    profile_id: Option<i64>,
    item_id: i64,
    app: tauri::AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<MotionPreview, &'static str> {
    let (database, store) = open_store(&app)?;
    let item = store
        .list_motion_library(workspace_id, profile_id)
        .map_err(|_| "motion_library_preview_failed")?
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or("motion_library_preview_failed")?;
    motion_preview(&database, &store, workspace_id, &item.settings, &stream)
}

#[tauri::command]
pub async fn list_motion_library_versions(
    workspace_id: i64,
    item_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::MotionVersion>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .motion_library_versions(workspace_id, item_id)
        .map_err(|_| "motion_library_versions_failed")
}

#[tauri::command]
pub async fn restore_motion_library_version(
    workspace_id: i64,
    item_id: i64,
    version: i64,
    app: tauri::AppHandle,
) -> Result<MotionLibraryItem, &'static str> {
    let (database, store) = open_store(&app)?;
    store
        .restore_motion_library_version(&database, workspace_id, item_id, version)
        .map_err(|_| "motion_library_restore_failed")
}

#[tauri::command]
pub async fn preview_motion_library_version(
    workspace_id: i64,
    item_id: i64,
    version: i64,
    app: tauri::AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<MotionPreview, &'static str> {
    let (database, store) = open_store(&app)?;
    let settings = store
        .motion_library_version(workspace_id, item_id, version)
        .map_err(|_| "motion_library_preview_failed")?
        .settings(None);
    motion_preview(&database, &store, workspace_id, &settings, &stream)
}

#[tauri::command]
pub async fn preview_run_motion_item(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    index: usize,
    app: tauri::AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<MotionPreview, &'static str> {
    let (database, store) = open_store(&app)?;
    let settings = store
        .run_motion_item(workspace_id, run_id, result_id, index)
        .map_err(|_| "motion_library_preview_failed")?;
    motion_preview(&database, &store, workspace_id, &settings, &stream)
}

#[tauri::command]
pub async fn save_motion_result(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    index: i64,
    store: State<'_, Store>,
) -> Result<MotionLibraryItem, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .save_motion_result(workspace_id, run_id, result_id, index)
        .map_err(|_| "motion_result_save_failed")
}

#[derive(serde::Serialize)]
pub struct SavedRunMotion {
    result_id: i64,
    index: i64,
    profile_id: Option<i64>,
}

#[tauri::command]
pub async fn saved_run_motion(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<SavedRunMotion>, &'static str> {
    Ok(store
        .lock()
        .map_err(|_| "store_unavailable")?
        .saved_run_motion(workspace_id, run_id)
        .map_err(|_| "motion_library_load_failed")?
        .into_iter()
        .map(|(result_id, index, profile_id)| SavedRunMotion {
            result_id,
            index,
            profile_id,
        })
        .collect())
}
