use arrumo_workspaces::{CompletedVideo, WorkspaceStore};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::Store;

#[tauri::command]
pub async fn get_completed_video(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<CompletedVideo, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .completed_video(workspace_id, run_id)
        .map_err(|_| "completed_video_failed")
}

#[tauri::command]
pub async fn rename_completed_video(
    workspace_id: i64,
    run_id: i64,
    title: String,
    store: State<'_, Store>,
) -> Result<String, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .rename_completed_video(workspace_id, run_id, &title)
        .map_err(|_| "rename_completed_video_failed")
}

#[tauri::command]
pub async fn export_completed_video(
    workspace_id: i64,
    run_id: i64,
    app: AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
        let Some(destination) = app
            .dialog()
            .file()
            .add_filter("ZIP", &["zip"])
            .set_file_name(format!("arrumo-{run_id}.zip"))
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let destination = destination
            .into_path()
            .map_err(|_| "export_completed_video_failed")?;
        store
            .export_generation_delivery(&database, workspace_id, run_id, &destination)
            .map_err(|_| "export_completed_video_failed")?;
        Ok(true)
    })
    .await
    .map_err(|_| "export_completed_video_failed")?
}

#[tauri::command]
pub async fn replay_completed_video(
    workspace_id: i64,
    run_id: i64,
    request_key: String,
    mode: arrumo_workspaces::ReplayMode,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .replay_generation(workspace_id, run_id, &request_key, mode)
        .map(|_| ())
        .map_err(|_| "replay_completed_video_failed")
}

#[tauri::command]
pub async fn add_completed_video_feedback(
    workspace_id: i64,
    run_id: i64,
    result_id: Option<i64>,
    markdown: String,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::CompletedVideoFeedback, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .add_completed_video_feedback(workspace_id, run_id, result_id, &markdown)
        .map_err(|_| "add_completed_video_feedback_failed")
}
