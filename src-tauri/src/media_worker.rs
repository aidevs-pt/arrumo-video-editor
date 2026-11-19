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
