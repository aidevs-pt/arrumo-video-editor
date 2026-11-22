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
mod backup;
mod completed_video;
mod delivery_stream;
mod desktop_job_workers;
mod guided_preview;
mod media_worker;
mod motion_library;
mod plugin_settings;
mod profiles;

type Store = Mutex<WorkspaceStore>;

#[tauri::command]
fn get_preview_settings(
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::PreviewSettings, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .preview_settings()
        .map_err(|_| "preview_settings_failed")
}

#[tauri::command]
fn set_preview_max_edge(
    max_edge: Option<u32>,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::PreviewSettings, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    store
        .set_preview_max_edge(max_edge)
        .and_then(|()| store.preview_settings())
        .map_err(|_| "preview_settings_failed")
}

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
fn set_workspace_appearance(
    workspace_id: i64,
    color: String,
    icon: String,
    image_asset_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<Workspace, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_workspace_appearance(workspace_id, &color, &icon, image_asset_id)
        .map_err(|_| "workspace_appearance_failed")
}

#[tauri::command]
fn rename_workspace(
    workspace_id: i64,
    name: String,
    store: State<'_, Store>,
) -> Result<Workspace, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .rename_workspace(workspace_id, &name)
        .map_err(|_| "workspace_rename_failed")
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
async fn list_footage(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_footage(workspace_id)
        .map_err(|_| "asset_list_failed")
}

#[tauri::command]
async fn import_asset(
    workspace_id: i64,
    source: PathBuf,
    thumbnail_reference: Option<bool>,
    footage: Option<bool>,
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
    } else if footage.unwrap_or(false) {
        store.import_footage(&root, workspace_id, &source)
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
async fn list_asset_organization(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<std::collections::BTreeMap<i64, arrumo_workspaces::AssetOrganization>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_asset_organization(workspace_id)
        .map_err(|_| "asset_organization_failed")
}

#[tauri::command]
async fn set_asset_organization(
    workspace_id: i64,
    asset_id: i64,
    settings: arrumo_workspaces::AssetOrganization,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::AssetOrganization, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_asset_organization(workspace_id, asset_id, settings)
        .map_err(|_| "asset_organization_failed")
}

#[tauri::command]
async fn replace_asset(
    app: AppHandle,
    workspace_id: i64,
    asset_id: i64,
    source: PathBuf,
) -> Result<WorkspaceAsset, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    tauri::async_runtime::spawn_blocking(move || {
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .replace_asset(&asset_root(&database), workspace_id, asset_id, &source)
            .map_err(|_| "asset_replacement_failed")
    })
    .await
    .map_err(|_| "asset_replacement_failed")?
}

#[tauri::command]
async fn remove_asset(
    app: AppHandle,
    workspace_id: i64,
    asset_id: i64,
    confirmed_name: String,
) -> Result<bool, &'static str> {
    let database = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "store_unavailable")?
        .join("workspaces.sqlite");
    tauri::async_runtime::spawn_blocking(move || {
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .remove_workspace_asset(&database, workspace_id, asset_id, &confirmed_name)
            .map_err(|_| "asset_removal_failed")
    })
    .await
    .map_err(|_| "asset_removal_failed")?
}

#[tauri::command]
async fn set_batch_asset_notes(
    workspace_id: i64,
    asset_ids: Vec<i64>,
    notes: String,
    store: State<'_, Store>,
) -> Result<Vec<WorkspaceAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_batch_asset_notes(workspace_id, &asset_ids, &notes)
        .map_err(|_| "asset_notes_failed")
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
#[allow(clippy::too_many_arguments)]
async fn generate_library_images(
    workspace_id: i64,
    prompt: String,
    aspect_ratio: String,
    count: usize,
    notes: String,
    provider: Option<String>,
    model: Option<String>,
    references: Option<Vec<i64>>,
    app: AppHandle,
) -> Result<Vec<i64>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .generate_library_images(
                &database,
                workspace_id,
                &prompt,
                &aspect_ratio,
                count,
                &notes,
                provider.as_deref(),
                model.as_deref(),
                &references.unwrap_or_default(),
            )
            .map(|images| images.into_iter().map(|image| image.id).collect())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "image_generation_failed".to_owned())?
}

#[tauri::command]
async fn provider_usage(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::ProviderUsage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .provider_usage(workspace_id)
            .map_err(|_| "usage_unavailable".to_owned())
    })
    .await
    .map_err(|_| "usage_unavailable".to_owned())?
}

/// A clicked system notification brings the window forward and asks the page to open the run
/// or finished video it named. Only those two in-app pages are accepted.
fn open_clicked_notifications(app: &AppHandle) {
    use tauri::Emitter;
    use tauri_plugin_notification::NotificationExt;

    let handle = app.clone();
    let _ = app.notification().on_action(move |performed| {
        if performed.action_id() != "tap" {
            return;
        }
        let Some(href) = performed
            .notification()
            .and_then(|notification| notification.extra().get("href"))
            .and_then(|value| value.as_str())
            .filter(|href| notice_page(href))
        else {
            return;
        };
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        let _ = handle.emit_to("main", "system-notification-open", href);
    });
}

fn notice_page(href: &str) -> bool {
    ["/completed-video?", "/generation-run?"]
        .iter()
        .any(|page| href.starts_with(page))
        && href.len() <= 128
        && href
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/-?&=".contains(&byte))
}

#[tauri::command]
async fn transform_audio_asset(
    workspace_id: i64,
    asset_id: i64,
    transform: arrumo_workspaces::AudioTransform,
    provider: Option<String>,
    voice: Option<String>,
    app: AppHandle,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .transform_audio_asset(
                &database,
                workspace_id,
                asset_id,
                transform,
                provider.as_deref(),
                voice.as_deref(),
            )
            .map(|(id, _)| id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "audio_transform_failed".to_owned())?
}

#[tauri::command]
async fn dub_asset(
    workspace_id: i64,
    asset_id: i64,
    language: String,
    provider: Option<String>,
    app: AppHandle,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .dub_asset(
                &database,
                workspace_id,
                asset_id,
                &language,
                provider.as_deref(),
            )
            .map(|(id, _)| id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "dubbing_failed".to_owned())?
}

#[tauri::command]
async fn list_dubbing_providers(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::DubbingProvider>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .dubbing_providers(workspace_id)
            .map_err(|_| "dubbing_unavailable".to_owned())
    })
    .await
    .map_err(|_| "dubbing_unavailable".to_owned())?
}

#[tauri::command]
async fn list_ready_plugins(
    workspace_id: Option<i64>,
    app: AppHandle,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .ready_plugins(workspace_id)
            .map_err(|_| "plugins_unavailable".to_owned())
    })
    .await
    .map_err(|_| "plugins_unavailable".to_owned())?
}

/// A PNG data URL of sample text drawn with a caption style at a format's shape.
#[tauri::command]
async fn preview_caption_style(
    style: arrumo_workspaces::CaptionStyle,
    width: u32,
    height: u32,
    text: String,
) -> Result<String, String> {
    use base64::Engine as _;
    tauri::async_runtime::spawn_blocking(move || {
        arrumo_workspaces::caption_preview(&style, width, height, &text)
            .map(|png| {
                format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(png)
                )
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "caption_preview_failed".to_owned())?
}

#[tauri::command]
async fn list_audio_transforms(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::AudioTransformProvider>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .audio_transform_providers(workspace_id)
            .map_err(|_| "audio_transforms_unavailable".to_owned())
    })
    .await
    .map_err(|_| "audio_transforms_unavailable".to_owned())?
}

#[tauri::command]
async fn list_speech_models(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::SpeechModel>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .speech_models(workspace_id)
            .map_err(|_| "speech_models_unavailable".to_owned())
    })
    .await
    .map_err(|_| "speech_models_unavailable".to_owned())?
}

#[tauri::command]
async fn list_image_models(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::ImageModel>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .image_models(workspace_id)
            .map_err(|_| "image_models_unavailable".to_owned())
    })
    .await
    .map_err(|_| "image_models_unavailable".to_owned())?
}

#[tauri::command]
async fn list_video_models(
    workspace_id: i64,
    app: AppHandle,
) -> Result<Vec<arrumo_workspaces::VideoModel>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .video_models(workspace_id)
            .map_err(|_| "video_models_unavailable".to_owned())
    })
    .await
    .map_err(|_| "video_models_unavailable".to_owned())?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn generate_library_video(
    workspace_id: i64,
    prompt: String,
    aspect_ratio: String,
    duration_seconds: Option<u32>,
    notes: String,
    provider: Option<String>,
    model: Option<String>,
    references: Option<Vec<i64>>,
    app: AppHandle,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .generate_library_video(
                &database,
                workspace_id,
                &prompt,
                &aspect_ratio,
                duration_seconds,
                &notes,
                provider.as_deref(),
                model.as_deref(),
                &references.unwrap_or_default(),
            )
            .map(|(id, _)| id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "video_generation_failed".to_owned())?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn generate_library_audio(
    workspace_id: i64,
    kind: arrumo_workspaces::AudioKind,
    prompt: String,
    seconds: f64,
    notes: String,
    provider: Option<String>,
    label: Option<String>,
    app: AppHandle,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .generate_library_audio(
                &database,
                workspace_id,
                kind,
                &prompt,
                seconds,
                &notes,
                provider.as_deref(),
                label.as_deref(),
            )
            .map(|(id, _)| id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "audio_generation_failed".to_owned())?
}

#[tauri::command]
async fn generate_library_speech(
    workspace_id: i64,
    text: String,
    notes: String,
    provider: Option<String>,
    model: Option<String>,
    app: AppHandle,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .generate_library_speech(
                &database,
                workspace_id,
                &text,
                &notes,
                provider.as_deref(),
                model.as_deref(),
            )
            .map(|(id, _)| id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "speech_generation_failed".to_owned())?
}

#[tauri::command]
async fn draft_profile(
    workspace_id: i64,
    name: String,
    about: String,
    app: AppHandle,
) -> Result<arrumo_workspaces::ProfileDraft, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .draft_profile(workspace_id, &name, &about)
            .map_err(|error| {
                match error.downcast_ref::<arrumo_workspaces::AgentRunError>() {
                    Some(arrumo_workspaces::AgentRunError::Unavailable) => "agent_unavailable",
                    Some(arrumo_workspaces::AgentRunError::TimedOut) => "agent_timed_out",
                    Some(_) => "agent_failed",
                    None => "profile_draft_failed",
                }
                .to_owned()
            })
    })
    .await
    .map_err(|_| "profile_draft_failed".to_owned())?
}

#[tauri::command]
async fn list_context_sources(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::ContextSource>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .context_sources(workspace_id)
        .map_err(|_| "context_sources_failed")
}

#[tauri::command]
async fn forget_context_source(
    workspace_id: i64,
    source: String,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .forget_context_source(workspace_id, &source)
        .map_err(|_| "context_sources_failed")
}

#[tauri::command]
async fn draft_workspace_context(
    workspace_id: i64,
    sources: Vec<String>,
    notes: String,
    app: AppHandle,
) -> Result<arrumo_workspaces::ContextDraft, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .draft_workspace_context(workspace_id, &sources, &notes)
            .map_err(|error| {
                match error.downcast_ref::<arrumo_workspaces::AgentRunError>() {
                    Some(arrumo_workspaces::AgentRunError::Unavailable) => "agent_unavailable",
                    Some(arrumo_workspaces::AgentRunError::TimedOut) => "agent_timed_out",
                    Some(_) => "agent_failed",
                    None => "context_draft_failed",
                }
                .to_owned()
            })
    })
    .await
    .map_err(|_| "context_draft_failed".to_owned())?
}

#[tauri::command]
async fn apply_context_import(
    workspace_id: i64,
    import: arrumo_workspaces::ContextImport,
    app: AppHandle,
) -> Result<arrumo_workspaces::ContextImportOutcome, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable")?
            .apply_context_import(&database, workspace_id, &import)
            .map_err(|_| "context_import_failed")
    })
    .await
    .map_err(|_| "context_import_failed")?
}

#[tauri::command]
async fn generate_motion_library_item(
    workspace_id: i64,
    profile_id: Option<i64>,
    request: String,
    based_on: Option<i64>,
    app: AppHandle,
) -> Result<arrumo_workspaces::MotionLibraryItem, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable".to_owned())?
            .join("workspaces.sqlite");
        WorkspaceStore::open(&database)
            .map_err(|_| "store_unavailable".to_owned())?
            .generate_motion_library_item(&database, workspace_id, profile_id, &request, based_on)
            .map_err(|error| {
                match error.downcast_ref::<arrumo_workspaces::AgentRunError>() {
                    Some(arrumo_workspaces::AgentRunError::Unavailable) => "agent_unavailable",
                    Some(arrumo_workspaces::AgentRunError::TimedOut) => "agent_timed_out",
                    Some(_) => "agent_failed",
                    None => "motion_generation_failed",
                }
                .to_owned()
            })
    })
    .await
    .map_err(|_| "motion_generation_failed".to_owned())?
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
async fn remove_workspace_feedback(
    workspace_id: i64,
    feedback_id: i64,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .remove_workspace_feedback(workspace_id, feedback_id)
        .map_err(|_| "feedback_remove_failed")
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

fn media_root(app: &AppHandle) -> Result<PathBuf, &'static str> {
    Ok(asset_root(
        &app.path()
            .app_local_data_dir()
            .map_err(|_| "asset_storage_unavailable")?
            .join("workspaces.sqlite"),
    ))
}

#[tauri::command]
async fn get_brand_fonts(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::BrandFont>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .brand_fonts(workspace_id)
        .map_err(|_| "brand_fonts_load_failed")
}

#[tauri::command]
async fn import_brand_font(
    workspace_id: i64,
    role: arrumo_workspaces::FontRole,
    weight: arrumo_workspaces::FontWeight,
    source: PathBuf,
    app: AppHandle,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::BrandFont, &'static str> {
    let root = media_root(&app)?;
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .import_brand_font(&root, workspace_id, role, weight, &source)
        .map_err(|error| {
            error
                .downcast_ref::<arrumo_workspaces::UnusableFont>()
                .map_or("brand_font_save_failed", |problem| problem.code)
        })
}

#[tauri::command]
async fn remove_brand_font(
    workspace_id: i64,
    role: arrumo_workspaces::FontRole,
    weight: arrumo_workspaces::FontWeight,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .remove_brand_font(workspace_id, role, weight)
        .map_err(|_| "brand_font_remove_failed")
}

/// A saved brand font as a data URL the page can load to show a specimen.
#[tauri::command]
async fn brand_font_data(
    workspace_id: i64,
    sha256: String,
    app: AppHandle,
    store: State<'_, Store>,
) -> Result<String, &'static str> {
    use base64::Engine as _;
    let root = media_root(&app)?;
    let bytes = store
        .lock()
        .map_err(|_| "store_unavailable")?
        .brand_font_bytes(&root, workspace_id, &sha256)
        .map_err(|_| "brand_fonts_load_failed")?;
    Ok(format!(
        "data:font/ttf;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
async fn get_brand_palette(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::BrandColor>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .brand_palette(workspace_id)
        .map_err(|_| "brand_colors_load_failed")
}

#[tauri::command]
async fn set_brand_color_details(
    workspace_id: i64,
    color: String,
    name: String,
    for_videos: bool,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::BrandColor>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_brand_color_details(workspace_id, &color, &name, for_videos)
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
    let store = store.lock().map_err(|_| "store_unavailable")?;
    let run = store
        .enqueue_video_draft(workspace_id, draft_id, &request_key, &options)
        .map_err(|_| "generation_enqueue_failed")?;
    store.track_usage(
        "video_queued",
        &[("autopilot", serde_json::json!(options.autopilot))],
    );
    Ok(run)
}

#[tauri::command]
fn read_privacy_settings(
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::PrivacySettings, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .privacy_settings()
        .map_err(|_| "privacy_settings_failed")
}

#[tauri::command]
fn write_privacy_settings(
    usage_data: bool,
    crash_reports: bool,
    store: State<'_, Store>,
) -> Result<arrumo_workspaces::PrivacySettings, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_privacy_settings(usage_data, crash_reports)
        .map_err(|_| "privacy_settings_failed")
}

#[tauri::command]
fn read_notification_settings(store: State<'_, Store>) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .system_notifications()
        .map_err(|_| "notification_settings_failed")
}

#[tauri::command]
fn write_notification_settings(
    system: bool,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_system_notifications(system)
        .map_err(|_| "notification_settings_failed")
}

/// Generation events after `after` in every workspace, plus the newest id, for the system
/// notification watcher. Pass `null` to only learn where to start.
#[tauri::command]
fn system_notices(
    after: Option<i64>,
    store: State<'_, Store>,
) -> Result<serde_json::Value, &'static str> {
    let store = store.lock().map_err(|_| "store_unavailable")?;
    let latest = store
        .latest_generation_notification()
        .map_err(|_| "notifications_failed")?;
    let notices = match after {
        Some(after) => store
            .system_notices_after(after)
            .map_err(|_| "notifications_failed")?,
        None => Vec::new(),
    };
    let enabled = store
        .system_notifications()
        .map_err(|_| "notifications_failed")?;
    Ok(serde_json::json!({"latest": latest, "enabled": enabled, "notices": notices}))
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
async fn read_all_completion_notifications(
    workspace_id: i64,
    store: State<'_, Store>,
) -> Result<usize, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .read_all_completion_notifications(workspace_id)
        .map_err(|_| "notification_read_failed")
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
async fn list_scheduled_outputs(
    workspace_id: i64,
    profile_id: Option<i64>,
    start: String,
    end: String,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::ScheduledOutput>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .scheduled_outputs(workspace_id, profile_id, &start, &end)
        .map_err(|_| "calendar_list_failed")
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
async fn delete_generation(
    workspace_id: i64,
    run_id: i64,
    confirmed_title: String,
    app: tauri::AppHandle,
) -> Result<bool, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let database = app
            .path()
            .app_local_data_dir()
            .map_err(|_| "store_unavailable")?
            .join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).map_err(|_| "store_unavailable")?;
        store
            .delete_generation(&database, workspace_id, run_id, &confirmed_title)
            .map_err(|_| "generation_delete_failed")
    })
    .await
    .map_err(|_| "generation_delete_failed")?
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
async fn agent_logs(
    workspace_id: i64,
    run_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<Vec<arrumo_workspaces::AgentLogSummary>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .agent_logs(workspace_id, run_id, 200)
        .map_err(|_| "agent_logs_failed")
}

#[tauri::command]
async fn agent_log(
    workspace_id: i64,
    log_id: i64,
    store: State<'_, Store>,
) -> Result<Option<arrumo_workspaces::AgentLog>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .agent_log(workspace_id, log_id)
        .map_err(|_| "agent_logs_failed")
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
    save_motion_indexes: Option<Vec<usize>>,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .review_production_result_with_motion(
            workspace_id,
            run_id,
            result_id,
            decision,
            &notes,
            save_motion_indexes.as_deref(),
        )
        .map_err(|_| "production_review_failed")
}

fn main() {
    // WebKitGTK's DMA-BUF renderer stalls media pipelines before demuxing on many Linux systems,
    // so videos never load and report no error. Users can still opt back in by setting the variable.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: runs first in main, before any thread exists.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(launch_options())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
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

            open_clicked_notifications(app.handle());

            let data_dir: PathBuf = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let database = data_dir.join("workspaces.sqlite");
            // A restored backup replaces the data before anything opens it.
            if let Err(error) = arrumo_workspaces::apply_pending_import(&database) {
                eprintln!("Arrumo could not restore the staged backup: {error}");
            }
            arrumo_workspaces::install_crash_reporter(database.clone());
            let store = WorkspaceStore::open(&database)?;
            store.track_usage("app_started", &[]);
            app.manage(Mutex::new(store));
            app.manage(desktop_job_workers::DesktopJobWorkers::start(
                app.handle().clone(),
                database.clone(),
            )?);
            tauri::async_runtime::spawn_blocking(move || {
                let Ok(store) = WorkspaceStore::open(&database) else {
                    eprintln!("Arrumo could not open the store to retry managed file cleanup.");
                    return;
                };
                if arrumo_workspaces::recover_motion_scratch(&database).is_err() {
                    eprintln!("Arrumo could not retry pending motion scratch cleanup.");
                }
                match store.recover_all_delivery_cleanup(&database) {
                    Ok(report) if report.pending_run_ids.is_empty() => {}
                    Ok(report) => eprintln!(
                        "Arrumo still has {} pending delivery cleanup entries.",
                        report.pending_run_ids.len()
                    ),
                    Err(_) => eprintln!("Arrumo could not retry pending delivery cleanup."),
                }
                match store.recover_asset_cleanup(&database) {
                    Ok(0) => {}
                    Ok(remaining) => {
                        eprintln!("Arrumo still has {remaining} pending asset cleanup entries.")
                    }
                    Err(_) => eprintln!("Arrumo could not retry pending asset cleanup."),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            profiles::set_profile_description,
            profiles::set_profile_format_context,
            profiles::list_profile_assets,
            profiles::list_asset_profile_shares,
            profiles::share_asset_with_profile,
            profiles::unshare_asset_from_profile,
            motion_library::list_motion_library,
            motion_library::preview_motion_library_item,
            motion_library::preview_run_motion_item,
            motion_library::saved_run_motion,
            motion_library::save_motion_result,
            motion_library::create_motion_library_item,
            motion_library::update_motion_library_item,
            motion_library::duplicate_motion_library_item,
            motion_library::delete_motion_library_item,
            motion_library::get_motion_library_shares,
            motion_library::set_motion_library_shares,
            list_workspaces,
            get_preview_settings,
            set_preview_max_edge,
            agent_settings::get_agent_settings,
            agent_settings::list_agent_plugins,
            plugin_settings::list_plugin_catalog,
            plugin_settings::set_plugin_active,
            plugin_settings::pending_plugin_tour,
            plugin_settings::plugin_tour,
            plugin_settings::finish_plugin_tour,
            plugin_settings::list_plugin_actions,
            plugin_settings::queue_plugin_action_job,
            plugin_settings::list_plugin_action_jobs,
            plugin_settings::process_plugin_action_job,
            plugin_settings::invoke_reviewed_plugin_action,
            plugin_settings::list_provider_preferences,
            plugin_settings::write_provider_preferences,
            backup::export_backup,
            read_privacy_settings,
            write_privacy_settings,
            read_notification_settings,
            write_notification_settings,
            system_notices,
            backup::stage_backup_import,
            backup::pending_backup_import,
            backup::cancel_backup_import,
            backup::restart_to_restore,
            plugin_settings::list_plugin_surfaces,
            plugin_settings::read_plugin_surface_context,
            plugin_settings::list_plugin_context_requests,
            plugin_settings::apply_plugin_context_grant,
            plugin_settings::read_plugin_settings,
            plugin_settings::write_plugin_setting,
            plugin_settings::read_plugin_credential_status,
            plugin_settings::write_plugin_credential,
            plugin_settings::import_plugin_package,
            plugin_settings::install_plugin_repository,
            plugin_settings::remove_plugin_package,
            plugin_settings::browse_plugin_registry,
            plugin_settings::install_registry_plugin,
            plugin_settings::open_plugin_repository,
            plugin_settings::open_external_link,
            plugin_settings::open_plugin_setting_help,
            agent_settings::set_agent_provider,
            agent_settings::agent_cli_detected,
            agent_settings::agent_login_status,
            agent_settings::open_agent_setup_guide,
            create_workspace,
            set_workspace_appearance,
            rename_workspace,
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
            list_footage,
            preview_workspace_image,
            set_asset_notes,
            set_batch_asset_notes,
            list_asset_organization,
            set_asset_organization,
            remove_asset,
            replace_asset,
            get_workspace_context,
            draft_workspace_context,
            list_context_sources,
            forget_context_source,
            draft_profile,
            generate_library_images,
            generate_library_speech,
            generate_library_audio,
            generate_library_video,
            list_image_models,
            list_speech_models,
            list_audio_transforms,
            preview_caption_style,
            list_ready_plugins,
            list_dubbing_providers,
            dub_asset,
            transform_audio_asset,
            list_video_models,
            provider_usage,
            generate_motion_library_item,
            apply_context_import,
            set_workspace_context,
            list_workspace_feedback,
            update_workspace_feedback,
            remove_workspace_feedback,
            get_brand_colors,
            set_brand_colors,
            get_brand_fonts,
            import_brand_font,
            remove_brand_font,
            brand_font_data,
            get_brand_palette,
            set_brand_color_details,
            list_brand_assets,
            attach_brand_asset,
            detach_brand_asset,
            list_video_drafts,
            create_video_draft,
            update_video_draft,
            delete_video_draft,
            enqueue_video_draft,
            list_generation_runs,
            list_scheduled_outputs,
            get_generation_run,
            pending_generation_reviews,
            generation_inbox,
            read_all_completion_notifications,
            control_generation,
            generation_activity,
            agent_logs,
            agent_log,
            delete_generation,
            generation_plan,
            production_progress,
            review_production_result,
            completed_video::get_completed_video,
            completed_video::completed_video_poster,
            completed_video::set_output_schedule,
            completed_video::read_completion_notification,
            completed_video::export_completed_subtitles,
            completed_video::rename_completed_video,
            completed_video::export_completed_video,
            completed_video::export_captioned_output,
            completed_video::delete_completed_video,
            completed_video::replay_completed_video,
            completed_video::add_completed_video_feedback,
            completed_video::preview_completed_output,
            completed_video::preview_completed_supplemental_media,
            completed_video::import_completed_video_variant,
            completed_video::attach_completed_media,
            completed_video::detach_completed_media,
            guided_preview::preview_pending_supplemental,
            guided_preview::review_supplemental_items,
            guided_preview::review_preview_formats,
            completed_video::read_completed_subtitle_revision,
            completed_video::save_completed_subtitle_revision,
            guided_preview::open_guided_preview,
            media_worker::process_desktop_media_stage,
            media_worker::submit_desktop_motion_artifacts,
            media_worker::desktop_motion_renderer_available,
            media_worker::render_desktop_run_motion,
            media_worker::generate_delivery_subtitles,
            media_worker::import_delivery_subtitles
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Arrumo")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<desktop_job_workers::DesktopJobWorkers>().stop();
            }
        });
}

/// `ARRUMO_NO_SPLASH=1` skips the launch face, for quicker debugging launches.
fn launch_options() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let builder = tauri::plugin::Builder::new("launch-options");
    if std::env::var_os("ARRUMO_NO_SPLASH").is_some_and(|value| value != "0") {
        builder
            .js_init_script("document.documentElement.dataset.noSplash = '';".to_string())
            .build()
    } else {
        builder.build()
    }
}
