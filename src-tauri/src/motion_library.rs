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

#[derive(serde::Serialize)]
pub struct MotionPreview {
    video_url: String,
    sound_url: Option<String>,
}

#[tauri::command]
pub async fn preview_motion_library_item(
    workspace_id: i64,
    profile_id: Option<i64>,
    item_id: i64,
    app: tauri::AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<MotionPreview, &'static str> {
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
    let item = store
        .list_motion_library(workspace_id, profile_id)
        .map_err(|_| failed)?
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or(failed)?;
    let assets = store.list_assets(workspace_id).map_err(|_| failed)?;
    let root = arrumo_workspaces::asset_root(&database);
    let media_url = |id, kind| {
        let asset = assets
            .iter()
            .find(|asset| asset.id == id && asset.kind == kind)
            .ok_or(failed)?;
        let extension = std::path::Path::new(&asset.name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = match extension.as_str() {
            "mp4" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            "ogg" => "audio/ogg",
            _ => return Err(failed),
        };
        stream
            .open_media(arrumo_workspaces::asset_path(&root, workspace_id, id), mime)
            .map_err(|_| failed)
    };
    Ok(MotionPreview {
        video_url: media_url(item.settings.preview_asset_id, "video")?,
        sound_url: item
            .settings
            .sound_asset_id
            .map(|id| media_url(id, "audio"))
            .transpose()?,
    })
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
