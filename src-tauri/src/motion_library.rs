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
