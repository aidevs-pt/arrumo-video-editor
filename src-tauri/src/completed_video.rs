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

/// Saves one format with its subtitles drawn into the picture where the creator chooses.
/// Returns whether a file was written; cancelling the dialog is not an error.
#[tauri::command]
pub async fn export_captioned_output(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
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
            .add_filter("MP4", &["mp4"])
            .set_file_name(format!("arrumo-{run_id}-{format_id}-captioned.mp4"))
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let destination = destination
            .into_path()
            .map_err(|_| "captioned_export_failed")?;
        // The dialog already confirmed any overwrite, so the finished file replaces it.
        let pending =
            destination.with_file_name(format!(".arrumo-captioned-{}.mp4", std::process::id()));
        let _ = std::fs::remove_file(&pending);
        let written = store
            .export_captioned_output(&database, workspace_id, run_id, format_id, &pending)
            .and_then(|()| Ok(std::fs::rename(&pending, &destination)?));
        if written.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        written.map_err(|_| "captioned_export_failed")?;
        Ok(true)
    })
    .await
    .map_err(|_| "captioned_export_failed")?
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
    languages: Vec<String>,
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
    let languages = store
        .completed_subtitle_languages(workspace_id, run_id, format_id)
        .map_err(|_| "delivery_preview_failed")?;
    let video_url = stream
        .open(video_path)
        .map_err(|_| "delivery_preview_failed")?;
    Ok(DeliveryPreview {
        video_url,
        languages,
        subtitles: subtitles.subtitles,
        revision: subtitles.revision,
    })
}

#[tauri::command]
pub async fn read_completed_subtitle_revision(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    revision: Option<i64>,
    language: Option<String>,
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
            .completed_subtitle_track(
                &database,
                workspace_id,
                run_id,
                format_id,
                language.as_deref(),
                revision,
            )
            .map_err(|_| "subtitle_revision_read_failed")
    })
    .await
    .map_err(|_| "subtitle_revision_read_failed")?
}

/// Saves one subtitle track (a revision, or the latest) where the creator chooses. Returns
/// whether a file was written; cancelling the dialog is not an error.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn export_completed_subtitles(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    revision: Option<i64>,
    language: Option<String>,
    file_name: String,
    app: AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let track = WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .completed_subtitle_track(
                &database,
                workspace_id,
                run_id,
                format_id,
                language.as_deref(),
                revision,
            )
            .map_err(|_| "subtitle_export_failed")?;
        let name: String = file_name
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .take(80)
            .collect();
        let name = if name.ends_with(".srt") {
            name
        } else {
            "subtitles.srt".to_owned()
        };
        let Some(destination) = app
            .dialog()
            .file()
            .add_filter("SRT", &["srt"])
            .set_file_name(name)
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let destination = destination
            .into_path()
            .map_err(|_| "subtitle_export_failed")?;
        std::fs::write(destination, track.subtitles).map_err(|_| "subtitle_export_failed")?;
        Ok(true)
    })
    .await
    .map_err(|_| "subtitle_export_failed")?
}

#[tauri::command]
pub async fn save_completed_subtitle_revision(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    expected_revision: i64,
    language: Option<String>,
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
            .revise_completed_subtitle_track(
                workspace_id,
                run_id,
                format_id,
                language.as_deref(),
                expected_revision,
                &subtitles,
            )
            .map_err(|_| "subtitle_revision_save_failed")
    })
    .await
    .map_err(|_| "subtitle_revision_save_failed")?
}

#[tauri::command]
pub async fn import_completed_video_variant(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    picker_title: String,
    app: AppHandle,
) -> Result<bool, &'static str> {
    let failed = "video_variant_import_failed";
    tauri::async_runtime::spawn_blocking(move || {
        let Some(source) = app
            .dialog()
            .file()
            .set_title(picker_title)
            .add_filter("MP4", &["mp4"])
            .blocking_pick_file()
        else {
            return Ok(false);
        };
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| failed)?
            .join("workspaces.sqlite");
        let source = source.into_path().map_err(|_| failed)?;
        WorkspaceStore::open(&database)
            .map_err(|_| failed)?
            .import_completed_video_variant(&database, workspace_id, run_id, format_id, &source)
            .map_err(|_| failed)?;
        Ok(true)
    })
    .await
    .map_err(|_| failed)?
}

#[tauri::command]
pub async fn attach_completed_media(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    asset_id: i64,
    app: AppHandle,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .attach_completed_media(&database, workspace_id, run_id, format_id, asset_id)
        .map_err(|_| "attach_completed_media_failed")
}

#[tauri::command]
pub async fn detach_completed_media(
    workspace_id: i64,
    run_id: i64,
    attachment_id: i64,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .detach_completed_media(workspace_id, run_id, attachment_id)
        .map_err(|_| "detach_completed_media_failed")
}

#[tauri::command]
pub async fn preview_completed_supplemental_media(
    workspace_id: i64,
    run_id: i64,
    index: Option<usize>,
    attachment_id: Option<i64>,
    app: AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<String, &'static str> {
    let failed = "supplemental_media_preview_failed";
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| failed)?
        .join("workspaces.sqlite");
    let store = WorkspaceStore::open(&database).map_err(|_| failed)?;
    let (entry, file) = match (index, attachment_id) {
        (Some(index), None) => {
            store.completed_supplemental_media_file(&database, workspace_id, run_id, index)
        }
        (None, Some(id)) => store.completed_attachment_file(&database, workspace_id, run_id, id),
        _ => return Err(failed),
    }
    .map_err(|_| failed)?;
    let mime = crate::delivery_stream::managed_media_type(&entry.name).ok_or(failed)?;
    stream.open_media(file, mime).map_err(|_| failed)
}

#[tauri::command]
pub async fn completed_video_poster(
    workspace_id: i64,
    run_id: i64,
    app: AppHandle,
    stream: State<'_, crate::delivery_stream::DeliveryStream>,
) -> Result<String, &'static str> {
    let failed = "completed_poster_failed";
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| failed)?
        .join("workspaces.sqlite");
    let (file, mime) = tauri::async_runtime::spawn_blocking(move || {
        let store = WorkspaceStore::open(&database).map_err(|_| failed)?;
        store
            .completed_poster(&database, workspace_id, run_id)
            .and_then(|(file, mime)| Ok((file.canonicalize()?, mime)))
            .map_err(|_| failed)
    })
    .await
    .map_err(|_| failed)??;
    stream.open_media(file, mime).map_err(|_| failed)
}
