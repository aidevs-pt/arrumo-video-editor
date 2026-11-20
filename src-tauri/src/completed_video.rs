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
pub async fn read_completion_notification(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .read_completion_notification(workspace_id, run_id)
        .map(|_| ())
        .map_err(|_| "notification_read_failed")
}

#[tauri::command]
pub async fn set_output_schedule(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    date: Option<String>,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_output_schedule(workspace_id, run_id, format_id, date.as_deref())
        .map_err(|_| "output_schedule_failed")
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
pub async fn delete_completed_video(
    workspace_id: i64,
    run_id: i64,
    confirmed_title: String,
    app: AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
        store
            .delete_completed_generation(&database, workspace_id, run_id, &confirmed_title)
            .map_err(|_| "completed_video_delete_failed")
    })
    .await
    .map_err(|_| "completed_video_delete_failed")?
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

#[derive(serde::Serialize)]
pub struct DeliveryPreview {
    video_url: String,
    subtitles: String,
    revision: i64,
}

#[tauri::command]
pub async fn preview_completed_output(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    app: AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<DeliveryPreview, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
    let (video_path, _) = store
        .verified_delivery_files(&database, workspace_id, run_id, format_id)
        .map_err(|_| "delivery_preview_failed")?;
    let subtitles = store
        .completed_subtitles(&database, workspace_id, run_id, format_id, None)
        .map_err(|_| "delivery_preview_failed")?;
    let video_url = stream
        .open(video_path)
        .map_err(|_| "delivery_preview_failed")?;
    Ok(DeliveryPreview {
        video_url,
        subtitles: subtitles.subtitles,
        revision: subtitles.revision,
    })
}

#[tauri::command]
pub async fn read_completed_subtitle_revision(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    revision: i64,
    app: AppHandle,
) -> Result<arrumo_workspaces::SubtitleRevision, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .completed_subtitles(&database, workspace_id, run_id, format_id, Some(revision))
            .map_err(|_| "subtitle_revision_read_failed")
    })
    .await
    .map_err(|_| "subtitle_revision_read_failed")?
}

#[tauri::command]
pub async fn save_completed_subtitle_revision(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    expected_revision: i64,
    subtitles: String,
    app: AppHandle,
) -> Result<i64, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .revise_completed_subtitles(
                workspace_id,
                run_id,
                format_id,
                expected_revision,
                &subtitles,
            )
            .map_err(|_| "subtitle_revision_save_failed")
    })
    .await
    .map_err(|_| "subtitle_revision_save_failed")?
}
