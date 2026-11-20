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
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .delete_motion_library_item(workspace_id, item_id, &confirmed_title)
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
