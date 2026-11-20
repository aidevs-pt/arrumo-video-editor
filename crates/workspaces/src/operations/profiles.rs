use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{MAX_PROFILE_DESCRIPTION, WorkspaceStore};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescriptionInput {
    workspace_id: i64,
    profile_id: i64,
    description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormatContextInput {
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileInput {
    workspace_id: i64,
    profile_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetInput {
    workspace_id: i64,
    asset_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShareInput {
    workspace_id: i64,
    asset_id: i64,
    profile_id: i64,
    format_id: Option<i64>,
}

pub(super) fn definitions() -> [Value; 6] {
    let id = json!({"type": "integer", "minimum": 1});
    let profile = json!({"type": "object", "properties": {"workspaceId": id, "profileId": id},
        "required": ["workspaceId", "profileId"], "additionalProperties": false});
    let share = json!({"type": "object", "properties": {
        "workspaceId": id, "assetId": id, "profileId": id,
        "formatId": {"type": ["integer", "null"], "minimum": 1,
            "description": "Limit the share to one enabled format. Omit to share with the whole profile."}
    }, "required": ["workspaceId", "assetId", "profileId"], "additionalProperties": false});
    [
        json!({"name": "set_profile_description", "description": "Replace a profile's one-line description shown in profile lists.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id,
                "description": {"type": "string", "maxLength": MAX_PROFILE_DESCRIPTION}
            }, "required": ["workspaceId", "profileId", "description"], "additionalProperties": false}}),
        json!({"name": "set_profile_format_context", "description": "Replace Markdown context that applies only when the profile renders one format. Empty text clears it.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id, "formatId": id, "markdown": {"type": "string"}
            }, "required": ["workspaceId", "profileId", "formatId", "markdown"], "additionalProperties": false}}),
        json!({"name": "list_profile_assets", "description": "List workspace assets shared with a profile, with the format each share is limited to.",
            "inputSchema": profile, "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_asset_profile_shares", "description": "List the profiles and formats a workspace asset is shared with.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "assetId": id},
                "required": ["workspaceId", "assetId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "share_asset_with_profile", "description": "Share a workspace asset with a profile or one of its formats. A profile-wide share replaces format shares.",
            "inputSchema": share.clone(), "annotations": {"idempotentHint": true}}),
        json!({"name": "unshare_asset_from_profile", "description": "Remove one asset share scope from a profile. The asset stays in the workspace library.",
            "inputSchema": share, "annotations": {"idempotentHint": true}}),
    ]
}

pub(super) fn call(
    store: &mut WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "set_profile_description" => parse(arguments).and_then(|args: DescriptionInput| {
            store
                .set_profile_description(args.workspace_id, args.profile_id, &args.description)
                .map(|profile| json!({"profile": profile}))
                .map_err(|_| "Could not save the description. Check the profile and keep it under 280 characters")
        }),
        "set_profile_format_context" => parse(arguments).and_then(|args: FormatContextInput| {
            store
                .set_profile_format_context(args.workspace_id, args.profile_id, args.format_id, &args.markdown)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save format context. Check the profile and that the format is enabled")
        }),
        "list_profile_assets" => parse(arguments).and_then(|args: ProfileInput| {
            store
                .list_profile_assets(args.workspace_id, args.profile_id)
                .map(|assets| json!({"assets": assets}))
                .map_err(|_| "Could not list profile assets")
        }),
        "list_asset_profile_shares" => parse(arguments).and_then(|args: AssetInput| {
            store
                .list_asset_profile_shares(args.workspace_id, args.asset_id)
                .map(|shares| json!({"shares": shares}))
                .map_err(|_| "Could not list asset shares")
        }),
        "share_asset_with_profile" => parse(arguments).and_then(|args: ShareInput| {
            store
                .share_asset_with_profile(args.workspace_id, args.asset_id, args.profile_id, args.format_id)
                .map(|()| json!({"shared": true}))
                .map_err(|_| "Could not share the asset. Check the asset, profile and format")
        }),
        "unshare_asset_from_profile" => parse(arguments).and_then(|args: ShareInput| {
            store
                .unshare_asset_from_profile(args.workspace_id, args.asset_id, args.profile_id, args.format_id)
                .map(|removed| json!({"removed": removed}))
                .map_err(|_| "Could not remove the share")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid profile arguments")
}
