use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{MotionArtifact, MotionLibrarySettings, WorkspaceStore};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct List {
    workspace_id: i64,
    profile_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Create {
    workspace_id: i64,
    settings: MotionLibrarySettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Update {
    workspace_id: i64,
    item_id: i64,
    settings: MotionLibrarySettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Generate {
    workspace_id: i64,
    profile_id: Option<i64>,
    based_on: Option<i64>,
    request: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Duplicate {
    workspace_id: i64,
    item_id: i64,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Delete {
    workspace_id: i64,
    item_id: i64,
    confirmed_title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadShares {
    workspace_id: i64,
    item_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetShares {
    workspace_id: i64,
    item_id: i64,
    profile_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveResult {
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    index: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitArtifacts {
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
    artifacts: Vec<MotionArtifact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderRun {
    workspace_id: i64,
    run_id: i64,
    plugin_id: String,
    package_sha256: String,
}

pub(super) fn definitions() -> [Value; 11] {
    let id = json!({"type": "integer", "minimum": 1});
    let optional_id = json!({"type": ["integer", "null"], "minimum": 1});
    let title = json!({"type": "string", "minLength": 1});
    let settings = json!({
        "type": "object",
        "properties": {
            "profile_id": optional_id, "kind": {"enum": ["animation", "transition"]},
            "title": title, "usage_markdown": {"type": "string"}, "preview_asset_id": id,
            "thumbnail_asset_id": optional_id, "sound_asset_id": optional_id
        },
        "required": ["kind", "title", "usage_markdown", "preview_asset_id"],
        "additionalProperties": false
    });
    [
        json!({"name": "list_motion_library", "description": "List workspace motion items and, if supplied, items scoped to the selected profile.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "profileId": optional_id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "create_motion_library_item", "description": "Create an animation or transition entry referencing existing workspace media.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "settings": settings},
                "required": ["workspaceId", "settings"], "additionalProperties": false}}),
        json!({"name": "update_motion_library_item", "description": "Replace an item's settings, scope, and media references.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id, "settings": settings},
                "required": ["workspaceId", "itemId", "settings"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "generate_motion_library_item", "description": "Let the workspace's agent design an animation or transition from a description and save its rendered preview to the library. Pass basedOn to add a changed version of an existing item; the original is kept. Takes minutes; read progress with list_agent_logs.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": optional_id, "basedOn": optional_id,
                "request": {"type": "string", "minLength": 1, "maxLength": 4000}
            }, "required": ["workspaceId", "request"], "additionalProperties": false}}),
        json!({"name": "duplicate_motion_library_item", "description": "Copy a motion item with a new title and reuse its media.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id, "title": title},
                "required": ["workspaceId", "itemId", "title"], "additionalProperties": false}}),
        json!({"name": "delete_motion_library_item", "description": "Delete a library entry using its exact saved title and keep its managed media.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id, "confirmedTitle": title},
                "required": ["workspaceId", "itemId", "confirmedTitle"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "get_motion_library_shares", "description": "Read the additional profile recipients of a motion library item.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id},
                "required": ["workspaceId", "itemId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_motion_library_shares", "description": "Replace a profile-scoped motion item's additional recipients. Pass an empty array to clear sharing.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "itemId": id, "profileIds": {"type": "array", "items": id, "uniqueItems": true}
            }, "required": ["workspaceId", "itemId", "profileIds"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "save_motion_result", "description": "Save a motion item from an accepted Assets stage result. Use the zero-based index in motion_items. Repeating the call returns the existing library entry.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id, "index": {"type": "integer", "minimum": 0}
            }, "required": ["workspaceId", "runId", "resultId", "index"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "render_run_motion", "description": "Render and submit a queued Assets stage with an installed motion renderer. Requires the Linux host's configured SDK worker.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "pluginId": title,
                "packageSha256": {"type": "string", "pattern": "^[a-fA-F0-9]{64}$"}
            }, "required": ["workspaceId", "runId", "pluginId", "packageSha256"], "additionalProperties": false}}),
        json!({"name": "submit_motion_artifacts", "description": "Import finished local renderer files and submit an Assets result under the current run lease.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attempt": id,
                "artifacts": {"type": "array", "minItems": 1, "items": {
                    "type": "object", "properties": {
                        "kind": {"enum": ["animation", "transition"]}, "title": title,
                        "usage_markdown": {"type": "string"}, "profile_id": optional_id,
                        "preview": {"type": "string", "minLength": 1, "description": "Absolute local video path"},
                        "thumbnail": {"type": ["string", "null"], "minLength": 1, "description": "Absolute local image path"},
                        "sound": {"type": ["string", "null"], "minLength": 1, "description": "Absolute local audio path"}
                    }, "required": ["kind", "title", "usage_markdown", "preview"], "additionalProperties": false
                }}
            }, "required": ["workspaceId", "runId", "attempt", "artifacts"], "additionalProperties": false}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    fn parse<T: serde::de::DeserializeOwned>(
        arguments: &Map<String, Value>,
    ) -> Result<T, &'static str> {
        serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid motion library arguments")
    }

    Some(match name {
        "list_motion_library" => parse(arguments).and_then(|args: List| {
            store.list_motion_library(args.workspace_id, args.profile_id)
                .map(|items| json!({"items": items}))
                .map_err(|_| "Could not list motion library. Check the workspace and profile")
        }),
        "create_motion_library_item" => parse(arguments).and_then(|args: Create| {
            store.create_motion_library_item(args.workspace_id, &args.settings)
                .map(|item| json!({"item": item}))
                .map_err(|_| "Could not create motion item. Check the title, scope, and media references")
        }),
        "update_motion_library_item" => parse(arguments).and_then(|args: Update| {
            store.update_motion_library_item(args.workspace_id, args.item_id, &args.settings)
                .map(|item| json!({"item": item}))
                .map_err(|_| "Could not update motion item. Check the item, scope, and media references")
        }),
        "generate_motion_library_item" => parse(arguments).and_then(|args: Generate| {
            store.generate_motion_library_item(database, args.workspace_id, args.profile_id, &args.request, args.based_on)
                .map(|item| json!({"item": item}))
                .map_err(|_| "Could not generate the animation. Read the latest agent log for the reason")
        }),
        "duplicate_motion_library_item" => parse(arguments).and_then(|args: Duplicate| {
            store.duplicate_motion_library_item(args.workspace_id, args.item_id, &args.title)
                .map(|item| json!({"item": item}))
                .map_err(|_| "Could not duplicate motion item. Check the item and title")
        }),
        "delete_motion_library_item" => parse(arguments).and_then(|args: Delete| {
            store.delete_motion_library_item(args.workspace_id, args.item_id, &args.confirmed_title)
                .map(|deleted| json!({"deleted": deleted}))
                .map_err(|_| "Could not delete motion item")
        }),
        "get_motion_library_shares" => parse(arguments).and_then(|args: ReadShares| {
            store.motion_library_shares(args.workspace_id, args.item_id)
                .map(|ids| json!({"profileIds": ids}))
                .map_err(|_| "Could not read shares. Check the workspace and item")
        }),
        "set_motion_library_shares" => parse(arguments).and_then(|args: SetShares| {
            store.set_motion_library_shares(args.workspace_id, args.item_id, &args.profile_ids)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save shares. Use distinct profiles in the workspace and omit the owning profile")
        }),
        "save_motion_result" => parse(arguments).and_then(|args: SaveResult| {
            store.save_motion_result(args.workspace_id, args.run_id, args.result_id, args.index)
                .map(|item| json!({"item": item}))
                .map_err(|_| "Could not save motion result. Check the run, accepted result, item index, and media references")
        }),
        "render_run_motion" => parse(arguments).and_then(|args: RenderRun| {
            let worker = crate::configured_motion_worker(None)
                .ok_or("The host motion renderer worker is unavailable")?;
            store.render_run_motion_with_worker(
                database, args.workspace_id, args.run_id, &args.plugin_id, &args.package_sha256, &worker,
            )
            .map(|id| json!({"resultId": id}))
            .map_err(|_| "Could not render motion. Check the queued Assets stage, installed package digest, and host worker")
        }),
        "submit_motion_artifacts" => parse(arguments).and_then(|args: SubmitArtifacts| {
            store.submit_motion_artifacts(database, args.workspace_id, args.run_id, args.attempt, &args.artifacts)
                .map(|id| json!({"resultId": id}))
                .map_err(|_| "Could not submit motion artifacts. Check the Assets-stage lease and local media files")
        }),
        _ => return None,
    })
}
