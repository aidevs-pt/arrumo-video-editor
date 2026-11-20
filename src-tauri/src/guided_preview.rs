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

#[derive(serde::Serialize)]
pub struct SupplementalReview {
    text: Vec<arrumo_workspaces::SupplementalText>,
    media: Vec<SupplementalReviewMedia>,
}

#[derive(serde::Serialize)]
struct SupplementalReviewMedia {
    #[serde(flatten)]
    entry: arrumo_workspaces::SupplementalMedia,
    url: Option<String>,
}

#[tauri::command]
pub async fn preview_pending_supplemental(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    app: AppHandle,
    stream: State<'_, DeliveryStream>,
) -> Result<SupplementalReview, &'static str> {
    let failed = "supplemental_review_failed";
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| failed)?
        .join("workspaces.sqlite");
    let store = WorkspaceStore::open(&database).map_err(|_| failed)?;
    let pending = store
        .pending_supplemental(workspace_id, run_id, result_id)
        .map_err(|_| failed)?;
    let media = pending
        .media
        .into_iter()
        .map(|entry| {
            let url = crate::delivery_stream::managed_media_type(&entry.name).and_then(|mime| {
                stream
                    .open_media(entry.managed_file(&database, workspace_id).ok()?, mime)
                    .ok()
            });
            SupplementalReviewMedia { entry, url }
        })
        .collect();
    Ok(SupplementalReview {
        text: pending.text,
        media,
    })
}

#[tauri::command]
pub async fn review_supplemental_items(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    review: arrumo_workspaces::SupplementalReview,
    notes: String,
    store: State<'_, crate::Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .review_supplemental_items(workspace_id, run_id, result_id, &review, &notes)
        .map_err(|_| "production_review_failed")
}

#[tauri::command]
pub async fn review_preview_formats(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    decisions: Vec<arrumo_workspaces::PreviewFormatDecision>,
    notes: String,
    store: State<'_, crate::Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .review_preview_formats(workspace_id, run_id, result_id, &decisions, &notes)
        .map_err(|_| "production_review_failed")
}
