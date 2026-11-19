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
