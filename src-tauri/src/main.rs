#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};

use arrumo_workspaces::{
    BrandAsset, BrandAssetRole, DeliveryOverrides, DimensionOverrides, DraftSettings,
    FormatDelivery, GenerationActivity, GenerationPlan, GenerationRun, OutputFormat,
    OutputFormatSettings, ProductionProgress, Profile, ProfileDelivery, ProfileFormat,
    ReviewDecision, RunControl, RunOptions, SupplementalOutputs, VideoDraft, Workspace,
    WorkspaceAsset, WorkspaceFeedback, WorkspaceStore, asset_root,
};
use tauri::{AppHandle, Manager, State};

mod agent_settings;
mod completed_video;
mod delivery_stream;
mod guided_preview;
mod media_worker;
mod plugin_settings;

type Store = Mutex<WorkspaceStore>;

#[tauri::command]
fn list_workspaces(store: State<'_, Store>) -> Result<Vec<Workspace>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list()
        .map_err(|_| "list_failed")
}

#[tauri::command]
fn create_workspace(name: String, store: State<'_, Store>) -> Result<Workspace, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create(&name)
        .map_err(|_| "create_failed")
}

#[tauri::command]
async fn list_profiles(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<Profile>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_profiles(workspace_id)
        .map_err(|_| "profiles_list_failed")
}

#[tauri::command]
async fn create_profile(
    workspace_id: i64,
    name: String,
    guidance: String,
    color: Option<String>,
    store: State<'_, Store>,
) -> Result<Profile, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_profile(workspace_id, &name, &guidance, color.as_deref())
        .map_err(|_| "profile_create_failed")
}

#[tauri::command]
async fn update_profile(
    workspace_id: i64,
    profile_id: i64,
    name: String,
    guidance: String,
    color: Option<String>,
    store: State<'_, Store>,
) -> Result<Profile, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_profile(workspace_id, profile_id, &name, &guidance, color.as_deref())
        .map_err(|_| "profile_update_failed")
}

#[tauri::command]
async fn list_output_formats(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<OutputFormat>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_output_formats(workspace_id)
        .map_err(|_| "format_list_failed")
}

#[tauri::command]
async fn create_output_format(
    workspace_id: i64,
    settings: OutputFormatSettings,
    store: State<'_, Store>,
) -> Result<OutputFormat, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_output_format(workspace_id, &settings)
        .map_err(|_| "format_create_failed")
}

#[tauri::command]
async fn update_output_format(
    workspace_id: i64,
    format_id: i64,
    settings: OutputFormatSettings,
    store: State<'_, Store>,
) -> Result<OutputFormat, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_output_format(workspace_id, format_id, &settings)
        .map_err(|_| "format_update_failed")
}

#[tauri::command]
async fn remove_output_format(
    workspace_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .remove_output_format(workspace_id, format_id)
        .map_err(|_| "format_remove_failed")
}

#[tauri::command]
async fn list_profile_formats(
    workspace_id: i64,
    profile_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<ProfileFormat>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_profile_formats(workspace_id, profile_id)
        .map_err(|_| "profile_format_list_failed")
}

#[tauri::command]
async fn set_profile_format_overrides(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    overrides: DimensionOverrides,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_format_overrides(workspace_id, profile_id, format_id, &overrides)
        .map_err(|_| "profile_format_save_failed")
}

#[tauri::command]
async fn get_format_delivery(
    workspace_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<FormatDelivery, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .format_delivery(workspace_id, format_id)
        .map_err(|_| "format_delivery_load_failed")
}

#[tauri::command]
async fn set_format_delivery(
    workspace_id: i64,
    format_id: i64,
    settings: SupplementalOutputs,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_format_delivery(workspace_id, format_id, &settings)
        .map_err(|_| "format_delivery_save_failed")
}

#[tauri::command]
async fn get_profile_delivery(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    store: State<'_, Store>,
) -> Result<ProfileDelivery, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .profile_delivery(workspace_id, profile_id, format_id)
        .map_err(|_| "profile_delivery_load_failed")
}

#[tauri::command]
async fn set_profile_delivery(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    overrides: DeliveryOverrides,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_delivery(workspace_id, profile_id, format_id, &overrides)
        .map_err(|_| "profile_delivery_save_failed")
}

#[tauri::command]
async fn list_assets(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_assets(workspace_id)
        .map_err(|_| "asset_list_failed")
}

#[tauri::command]
async fn import_asset(
    workspace_id: i64,
    source: PathBuf,
    thumbnail_reference: Option<bool>,
    app: AppHandle,
    store: State<'_, Store>,
) -> Result<WorkspaceAsset, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "asset_storage_unavailable")?
        .join("workspaces.sqlite");
    let root = asset_root(&database);
    let mut store = store.lock().map_err(|_| "store_unavailable")?;
    let imported = if thumbnail_reference.unwrap_or(false) {
        store.import_thumbnail_reference(&root, workspace_id, &source)
    } else {
        store.import_asset(&root, workspace_id, &source)
    };
    imported.map_err(|_| "asset_import_failed")
}

#[tauri::command]
async fn preview_workspace_image(
    workspace_id: i64,
    asset_id: i64,
    app: AppHandle,
) -> Result<Option<String>, &'static str> {
    use base64::{Engine, engine::general_purpose::STANDARD};

    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "asset_preview_failed")?
            .join("workspaces.sqlite");
        let database = database
            .canonicalize()
            .map_err(|_| "asset_preview_failed")?;
        let store = WorkspaceStore::open(&database).map_err(|_| "asset_preview_failed")?;
        store
            .image_preview(&asset_root(&database), workspace_id, asset_id)
            .map(|preview| {
                preview.map(|image| {
                    format!(
                        "data:{};base64,{}",
                        image.mime,
                        STANDARD.encode(image.bytes)
                    )
                })
            })
            .map_err(|_| "asset_preview_failed")
    })
    .await
    .map_err(|_| "asset_preview_failed")?
}

#[tauri::command]
async fn set_asset_notes(
    workspace_id: i64,
    asset_id: i64,
    notes: String,
    store: State<'_, Store>,
) -> Result<WorkspaceAsset, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_asset_notes(workspace_id, asset_id, &notes)
        .map_err(|_| "asset_notes_failed")
}

#[tauri::command]
async fn get_workspace_context(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Option<String>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .workspace_context(workspace_id)
        .map_err(|_| "workspace_context_load_failed")
}

#[tauri::command]
async fn set_workspace_context(
    workspace_id: i64,
    markdown: String,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_workspace_context(workspace_id, &markdown)
        .map_err(|_| "workspace_context_save_failed")
}

#[tauri::command]
async fn list_workspace_feedback(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceFeedback>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_workspace_feedback(workspace_id)
        .map_err(|_| "feedback_load_failed")
}

#[tauri::command]
async fn update_workspace_feedback(
    workspace_id: i64,
    feedback_id: i64,
    title: String,
    markdown: String,
    store: State<'_, Store>,
) -> Result<WorkspaceFeedback, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_workspace_feedback(workspace_id, feedback_id, &title, &markdown)
        .map_err(|_| "feedback_save_failed")
}

#[tauri::command]
async fn get_brand_colors(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<String>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .brand_colors(workspace_id)
        .map_err(|_| "brand_colors_load_failed")
}

#[tauri::command]
async fn set_brand_colors(
    workspace_id: i64,
    colors: Vec<String>,
    store: State<'_, Store>,
) -> Result<Vec<String>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_brand_colors(workspace_id, &colors)
        .map_err(|_| "brand_colors_save_failed")
}

#[tauri::command]
async fn list_brand_assets(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<BrandAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_brand_assets(workspace_id)
        .map_err(|_| "brand_assets_load_failed")
}

#[tauri::command]
async fn attach_brand_asset(
    workspace_id: i64,
    asset_id: i64,
    role: BrandAssetRole,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .attach_brand_asset(workspace_id, asset_id, role)
        .map_err(|_| "brand_asset_attach_failed")
}

#[tauri::command]
async fn detach_brand_asset(
    workspace_id: i64,
    asset_id: i64,
    role: BrandAssetRole,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .detach_brand_asset(workspace_id, asset_id, role)
        .map_err(|_| "brand_asset_detach_failed")
}

#[tauri::command]
async fn list_video_drafts(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<VideoDraft>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_video_drafts(workspace_id)
        .map_err(|_| "drafts_load_failed")
}

#[tauri::command]
async fn create_video_draft(
    workspace_id: i64,
    settings: DraftSettings,
    store: State<'_, Store>,
) -> Result<VideoDraft, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .create_video_draft(workspace_id, settings)
        .map_err(|_| "draft_create_failed")
}

#[tauri::command]
async fn delete_video_draft(
    workspace_id: i64,
    draft_id: i64,
    confirmed_title: String,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .delete_video_draft(workspace_id, draft_id, &confirmed_title)
        .map_err(|_| "draft_delete_failed")
}

#[tauri::command]
async fn update_video_draft(
    workspace_id: i64,
    draft_id: i64,
    settings: DraftSettings,
    store: State<'_, Store>,
) -> Result<VideoDraft, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .update_video_draft(workspace_id, draft_id, settings)
        .map_err(|_| "draft_update_failed")
}

#[tauri::command]
async fn enqueue_video_draft(
    workspace_id: i64,
    draft_id: i64,
    request_key: String,
    options: RunOptions,
    store: State<'_, Store>,
) -> Result<GenerationRun, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .enqueue_video_draft(workspace_id, draft_id, &request_key, &options)
        .map_err(|_| "generation_enqueue_failed")
}

#[tauri::command]
async fn pending_generation_reviews(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::PendingReview>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .pending_generation_reviews(workspace_id)
        .map_err(|_| "generation_review_list_failed")
}

#[tauri::command]
async fn generation_inbox(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::GenerationInbox, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .generation_inbox(workspace_id)
        .map_err(|_| "generation_inbox_failed")
}

#[tauri::command]
async fn get_generation_run(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<Option<GenerationRun>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .generation_run(workspace_id, run_id)
        .map_err(|_| "generation_run_failed")
}

#[tauri::command]
async fn list_generation_runs(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<GenerationRun>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_generation_runs(workspace_id)
        .map_err(|_| "generation_list_failed")
}

#[tauri::command]
async fn control_generation(
    workspace_id: i64,
    run_id: i64,
    action: RunControl,
    store: State<'_, Store>,
) -> Result<GenerationRun, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .control_generation(workspace_id, run_id, action)
        .map_err(|_| "generation_control_failed")
}

#[tauri::command]
async fn generation_activity(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<GenerationActivity>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .generation_activity(workspace_id, run_id)
        .map_err(|_| "generation_activity_failed")
}

#[tauri::command]
async fn generation_plan(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<Option<GenerationPlan>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .generation_plan(workspace_id, run_id)
        .map_err(|_| "generation_plan_failed")
}

#[tauri::command]
async fn production_progress(
    workspace_id: i64,
    run_id: i64,
    store: State<'_, Store>,
) -> Result<ProductionProgress, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .production_progress(workspace_id, run_id)
        .map_err(|_| "production_progress_failed")
}

#[tauri::command]
async fn review_production_result(
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    decision: ReviewDecision,
    notes: String,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .review_production_result(workspace_id, run_id, result_id, decision, &notes)
        .map_err(|_| "production_review_failed")
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("arrumo-panel", plugin_settings::serve_plugin_surface)
        .manage(delivery_stream::DeliveryStream::default())
        .setup(|app| {
            #[cfg(any(target_os = "linux", all(debug_assertions, target_os = "windows")))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(error) = app.deep_link().register_all() {
                    eprintln!("Arrumo deep-link registration failed: {error}");
                }
            }

            let data_dir: PathBuf = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Mutex::new(WorkspaceStore::open(
                data_dir.join("workspaces.sqlite"),
            )?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            agent_settings::get_agent_settings,
            agent_settings::list_agent_plugins,
            plugin_settings::list_plugin_catalog,
            plugin_settings::list_plugin_actions,
            plugin_settings::invoke_reviewed_plugin_action,
            plugin_settings::list_provider_preferences,
            plugin_settings::list_plugin_surfaces,
            plugin_settings::read_plugin_surface_context,
            plugin_settings::list_plugin_context_requests,
            plugin_settings::apply_plugin_context_grant,
            plugin_settings::read_plugin_settings,
            plugin_settings::write_plugin_setting,
            plugin_settings::read_plugin_credential_status,
            plugin_settings::write_plugin_credential,
            plugin_settings::import_plugin_package,
            plugin_settings::remove_plugin_package,
            plugin_settings::browse_plugin_registry,
            plugin_settings::install_registry_plugin,
            plugin_settings::open_plugin_repository,
            agent_settings::set_agent_provider,
            agent_settings::agent_cli_detected,
            agent_settings::agent_login_status,
            agent_settings::open_agent_setup_guide,
            create_workspace,
            list_profiles,
            create_profile,
            update_profile,
            list_output_formats,
            create_output_format,
            update_output_format,
            remove_output_format,
            list_profile_formats,
            set_profile_format_overrides,
            get_format_delivery,
            set_format_delivery,
            get_profile_delivery,
            set_profile_delivery,
            list_assets,
            import_asset,
            preview_workspace_image,
            set_asset_notes,
            get_workspace_context,
            set_workspace_context,
            list_workspace_feedback,
            update_workspace_feedback,
            get_brand_colors,
            set_brand_colors,
            list_brand_assets,
            attach_brand_asset,
            detach_brand_asset,
            list_video_drafts,
            create_video_draft,
            update_video_draft,
            delete_video_draft,
            enqueue_video_draft,
            list_generation_runs,
            get_generation_run,
            pending_generation_reviews,
            generation_inbox,
            control_generation,
            generation_activity,
            generation_plan,
            production_progress,
            review_production_result,
            completed_video::get_completed_video,
            completed_video::read_completion_notification,
            completed_video::rename_completed_video,
            completed_video::export_completed_video,
            completed_video::replay_completed_video,
            completed_video::add_completed_video_feedback,
            completed_video::preview_completed_output,
            guided_preview::open_guided_preview,
            media_worker::process_desktop_media_stage,
            media_worker::import_delivery_subtitles
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Arrumo");
}
