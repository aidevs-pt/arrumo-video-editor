use arrumo_workspaces::WorkspaceStore;
use tauri::{AppHandle, Manager, State};

use crate::delivery_stream::DeliveryStream;

#[tauri::command]
pub async fn open_guided_preview(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    format_id: i64,
    app: AppHandle,
    stream: State<'_, DeliveryStream>,
) -> Result<String, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
    let file = store
        .review_preview_file(&database, workspace_id, run_id, result_id, format_id)
        .map_err(|_| "guided_preview_failed")?;
    stream.open(file).map_err(|_| "guided_preview_failed")
}
