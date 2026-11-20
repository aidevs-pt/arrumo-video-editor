use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{AssetOrganization, BrandAssetRole, WorkspaceStore, asset_root};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportInput {
    workspace_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NotesInput {
    workspace_id: i64,
    asset_id: i64,
    notes: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrandAssetInput {
    workspace_id: i64,
    asset_id: i64,
    role: BrandAssetRole,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchNotesInput {
    workspace_id: i64,
    asset_ids: Vec<i64>,
    notes: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetInput {
    workspace_id: i64,
    asset_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OrganizationInput {
    workspace_id: i64,
    asset_id: i64,
    settings: AssetOrganization,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveAssetInput {
    workspace_id: i64,
    asset_id: i64,
    confirmed_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplaceAssetInput {
    workspace_id: i64,
    asset_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttachCompletedMedia {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    asset_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetachCompletedMedia {
    workspace_id: i64,
    run_id: i64,
    attachment_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportCompletedVideoVariant {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    source: String,
}

pub(super) fn definitions() -> [Value; 15] {
    let id = json!({"type": "integer", "minimum": 1});
    let assignment = json!({"type": "object", "properties": {
        "workspaceId": id, "assetId": id, "role": {"enum": ["identity", "style"]}
    }, "required": ["workspaceId", "assetId", "role"], "additionalProperties": false});
    [
        json!({"name": "detach_completed_media",
            "description": "Remove a post-completion attachment from a verified output while retaining the managed asset.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attachmentId": id
            }, "required": ["workspaceId", "runId", "attachmentId"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "import_completed_video_variant",
            "description": "Import a local MP4 as an alternate video for a verified completed output. The host verifies video dimensions, duration and audio before committing the asset and attachment.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "formatId", "source"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "attach_completed_media",
            "description": "Attach an existing managed asset to one verified output. Repeating a successful attachment returns added=false.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "assetId": id
            }, "required": ["workspaceId", "runId", "formatId", "assetId"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "list_workspace_assets", "description": "List managed assets and their IDs in a workspace.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "import_workspace_asset", "description": "Copy an existing media file at an absolute path on the local machine into managed workspace storage.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "source"], "additionalProperties": false}}),
        json!({"name": "import_thumbnail_reference", "description": "Import a local image as a thumbnail reference in managed workspace storage.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "source"], "additionalProperties": false}}),
        json!({"name": "set_asset_notes", "description": "Replace the usage notes for a managed asset. Pass an empty string to clear them.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "notes": {"type": "string"}
            }, "required": ["workspaceId", "assetId", "notes"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "set_batch_asset_notes", "description": "Replace usage notes on multiple assets in one workspace. All changes are applied together. Pass empty notes to clear them.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetIds": {"type": "array", "items": id, "minItems": 1, "uniqueItems": true},
                "notes": {"type": "string"}
            }, "required": ["workspaceId", "assetIds", "notes"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "list_brand_assets", "description": "List image assets assigned to workspace identity and style.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "attach_brand_asset", "description": "Assign a managed workspace image to identity or style.",
            "inputSchema": assignment, "annotations": {"idempotentHint": true}}),
        json!({"name": "detach_brand_asset", "description": "Remove a brand role from an image while keeping the managed asset.",
            "inputSchema": assignment, "annotations": {"idempotentHint": true}}),
        json!({"name": "get_asset_organization", "description": "Read an asset's group and labels.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "assetId": id},
                "required": ["workspaceId", "assetId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_asset_organization", "description": "Replace an asset's group and labels. An empty settings object clears both.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "settings": {"type": "object", "properties": {
                    "group": {"type": ["string", "null"]}, "labels": {"type": "array", "items": {"type": "string"}}
                }, "additionalProperties": false}
            }, "required": ["workspaceId", "assetId", "settings"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "replace_workspace_asset", "description": "Replace an active workspace asset with a same-kind local file. Live references follow the new asset ID and captured run inputs keep the previous media.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "assetId", "source"], "additionalProperties": false},
            "annotations": {"destructiveHint": true}}),
        json!({"name": "remove_workspace_asset", "description": "Remove an unused workspace asset and its managed copy after confirming its exact filename.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "confirmedName": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "assetId", "confirmedName"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
    ]
}

pub(super) fn call(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "detach_completed_media" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed media arguments")
            .and_then(|args: DetachCompletedMedia| {
                store.detach_completed_media(args.workspace_id, args.run_id, args.attachment_id)
                    .map(|removed| json!({"removed": removed}))
                    .map_err(|_| "Could not detach completed media")
            }),
        "import_completed_video_variant" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed video variant arguments")
            .and_then(|args: ImportCompletedVideoVariant| {
                let source = Path::new(&args.source);
                if !source.is_absolute() {
                    return Err("Variant source must be an absolute local path");
                }
                store.import_completed_video_variant(database, args.workspace_id, args.run_id, args.format_id, source)
                    .map(|asset| json!(asset))
                    .map_err(|_| "Could not import the MP4 variant for the verified output")
            }),
        "attach_completed_media" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed media arguments")
            .and_then(|args: AttachCompletedMedia| {
                store.attach_completed_media(database, args.workspace_id, args.run_id, args.format_id, args.asset_id)
                    .map(|added| json!({"added": added}))
                    .map_err(|_| "Could not attach media to the verified output")
            }),
        "list_workspace_assets" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: ListInput| {
                store
                    .list_assets(args.workspace_id)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not list workspace assets")
            }),
        "import_workspace_asset" | "import_thumbnail_reference" => {
            serde_json::from_value(Value::Object(arguments.clone()))
                .map_err(|_| "Invalid workspace asset arguments")
                .and_then(|args: ImportInput| {
                    let source = Path::new(&args.source);
                    if !source.is_absolute() {
                        return Err("Asset source must be an absolute local path");
                    }
                    let root = asset_root(database);
                    let asset = if name == "import_thumbnail_reference" {
                        store.import_thumbnail_reference(&root, args.workspace_id, source)
                    } else {
                        store.import_asset(&root, args.workspace_id, source)
                    };
                    asset
                        .map(|asset| json!({"asset": asset}))
                        .map_err(|_| "Could not import asset. Check the workspace and media file")
                })
        }
        "set_asset_notes" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: NotesInput| {
                store
                    .set_asset_notes(args.workspace_id, args.asset_id, &args.notes)
                    .map(|asset| json!({"asset": asset}))
                    .map_err(|_| "Could not update usage notes for this asset")
            }),
        "set_batch_asset_notes" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid batch asset notes arguments")
            .and_then(|args: BatchNotesInput| {
                store
                    .set_batch_asset_notes(args.workspace_id, &args.asset_ids, &args.notes)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not save notes for all selected workspace assets")
            }),
        "list_brand_assets" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: ListInput| {
                store
                    .list_brand_assets(args.workspace_id)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not list brand assets")
            }),
        "attach_brand_asset" | "detach_brand_asset" => {
            serde_json::from_value(Value::Object(arguments.clone()))
                .map_err(|_| "Invalid brand asset arguments")
                .and_then(|args: BrandAssetInput| {
                    if name == "attach_brand_asset" {
                        store
                            .attach_brand_asset(args.workspace_id, args.asset_id, args.role)
                            .map(|()| json!({"saved": true}))
                            .map_err(|_| "Could not assign brand role. Choose a workspace image")
                    } else {
                        store
                            .detach_brand_asset(args.workspace_id, args.asset_id, args.role)
                            .map(|removed| json!({"removed": removed}))
                            .map_err(|_| "Could not remove brand role")
                    }
                })
        }
        "get_asset_organization" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset organization arguments")
            .and_then(|args: AssetInput| {
                store
                    .asset_organization(args.workspace_id, args.asset_id)
                    .map(|settings| json!({"organization": settings}))
                    .map_err(|_| "Could not read asset organization")
            }),
        "set_asset_organization" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset organization arguments")
            .and_then(|args: OrganizationInput| {
                store
                    .set_asset_organization(args.workspace_id, args.asset_id, args.settings)
                    .map(|settings| json!({"organization": settings}))
                    .map_err(|_| "Could not save asset group and labels")
            }),
        "replace_workspace_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset replacement arguments")
            .and_then(|args: ReplaceAssetInput| {
                let source = Path::new(&args.source);
                if !source.is_absolute() {
                    return Err("Asset source must be an absolute local path");
                }
                store
                    .replace_asset(&asset_root(database), args.workspace_id, args.asset_id, source)
                    .map(|asset| json!({"asset": asset}))
                    .map_err(|_| "Could not replace asset. Check the workspace, active asset ID, and replacement media kind")
            }),
        "remove_workspace_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset removal arguments")
            .and_then(|args: RemoveAssetInput| {
                store
                    .remove_workspace_asset(
                        database,
                        args.workspace_id,
                        args.asset_id,
                        &args.confirmed_name,
                    )
                    .map(|removed| json!({"removed": removed}))
                    .map_err(
                        |_| "Could not finish asset removal. Retry with the same IDs and filename",
                    )
            }),
        _ => return None,
    })
}
