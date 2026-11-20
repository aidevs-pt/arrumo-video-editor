use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{MotionLibrarySettings, WorkspaceStore};

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

pub(super) fn definitions() -> [Value; 5] {
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
        json!({"name": "duplicate_motion_library_item", "description": "Copy a motion item with a new title and reuse its media.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id, "title": title},
                "required": ["workspaceId", "itemId", "title"], "additionalProperties": false}}),
        json!({"name": "delete_motion_library_item", "description": "Delete a library entry using its exact saved title and keep its managed media.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "itemId": id, "confirmedTitle": title},
                "required": ["workspaceId", "itemId", "confirmedTitle"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
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
        _ => return None,
    })
}
