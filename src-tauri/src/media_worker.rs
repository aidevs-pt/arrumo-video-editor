use arrumo_workspaces::{ProductionStage, WorkspaceStore};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn process_desktop_media_stage(
    workspace_id: i64,
    run_id: i64,
    stage: ProductionStage,
    app: AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .process_media_stage_at(&database, workspace_id, run_id, stage)
            .map(|work| work.is_some())
            .map_err(|_| "media_stage_failed")
    })
    .await
    .map_err(|_| "media_stage_failed")?
}

#[tauri::command]
pub async fn import_delivery_subtitles(
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    picker_title: String,
    app: AppHandle,
) -> Result<bool, &'static str> {
    use tauri_plugin_dialog::DialogExt;

    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
        let Some(source) = app
            .dialog()
            .file()
            .set_title(picker_title)
            .add_filter("SRT", &["srt"])
            .blocking_pick_file()
        else {
            return Ok(false);
        };
        let source = source.into_path().map_err(|_| "subtitle_import_failed")?;
        store
            .import_delivery_subtitles(&database, workspace_id, run_id, format_id, &source)
            .map_err(|_| "subtitle_import_failed")?;
        Ok(true)
    })
    .await
    .map_err(|_| "subtitle_import_failed")?
}

#[tauri::command]
pub async fn submit_desktop_motion_artifacts(
    workspace_id: i64,
    run_id: i64,
    artifacts: Vec<arrumo_workspaces::MotionArtifact>,
    app: AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .publish_queued_motion_artifacts(&database, workspace_id, run_id, &artifacts)
            .map(|result| result.is_some())
            .map_err(|_| "motion_artifacts_failed")
    })
    .await
    .map_err(|_| "motion_artifacts_failed")?
}

fn configured_motion_worker() -> Option<std::path::PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let path = std::path::PathBuf::from(std::env::var_os("ARRUMO_MOTION_SDK_WORKER")?);
    (path.is_absolute() && path.is_file()).then_some(path)
}

#[tauri::command]
pub fn desktop_motion_renderer_available() -> bool {
    configured_motion_worker().is_some()
}

#[tauri::command]
pub async fn render_desktop_run_motion(
    workspace_id: i64,
    run_id: i64,
    plugin_id: String,
    digest: String,
    app: AppHandle,
) -> Result<i64, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let worker = configured_motion_worker().ok_or("motion_renderer_unavailable")?;
        let toolchain = std::env::var_os("ARRUMO_MOTION_TOOLCHAIN").map(std::path::PathBuf::from);
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let job = tempfile::tempdir().map_err(|_| "motion_renderer_failed")?;
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .render_run_motion(
                &database,
                workspace_id,
                run_id,
                &plugin_id,
                &digest,
                &arrumo_workspaces::MotionWorkerPaths {
                    worker: &worker,
                    output: &job.path().join("output"),
                    toolchain: toolchain.as_deref(),
                },
            )
            .map_err(|_| "motion_renderer_failed")
    })
    .await
    .map_err(|_| "motion_renderer_failed")?
}
