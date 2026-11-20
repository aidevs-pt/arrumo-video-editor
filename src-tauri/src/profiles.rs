use arrumo_workspaces::{AssetProfileShare, Profile, ProfileAsset};
use tauri::State;

use crate::Store;

#[tauri::command]
pub async fn set_profile_description(
    workspace_id: i64,
    profile_id: i64,
    description: String,
    store: State<'_, Store>,
) -> Result<Profile, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_description(workspace_id, profile_id, &description)
        .map_err(|_| "profile_description_failed")
}

#[tauri::command]
pub async fn set_profile_format_context(
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    markdown: String,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .set_profile_format_context(workspace_id, profile_id, format_id, &markdown)
        .map_err(|_| "profile_format_context_failed")
}

#[tauri::command]
pub async fn list_profile_assets(
    workspace_id: i64,
    profile_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<ProfileAsset>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_profile_assets(workspace_id, profile_id)
        .map_err(|_| "profile_assets_failed")
}

#[tauri::command]
pub async fn list_asset_profile_shares(
    workspace_id: i64,
    asset_id: i64,
    store: State<'_, Store>,
) -> Result<Vec<AssetProfileShare>, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .list_asset_profile_shares(workspace_id, asset_id)
        .map_err(|_| "asset_shares_failed")
}

#[tauri::command]
pub async fn share_asset_with_profile(
    workspace_id: i64,
    asset_id: i64,
    profile_id: i64,
    format_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<(), &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .share_asset_with_profile(workspace_id, asset_id, profile_id, format_id)
        .map_err(|_| "asset_share_failed")
}

#[tauri::command]
pub async fn unshare_asset_from_profile(
    workspace_id: i64,
    asset_id: i64,
    profile_id: i64,
    format_id: Option<i64>,
    store: State<'_, Store>,
) -> Result<bool, &'static str> {
    store
        .lock()
        .map_err(|_| "store_unavailable")?
        .unshare_asset_from_profile(workspace_id, asset_id, profile_id, format_id)
        .map_err(|_| "asset_unshare_failed")
}
