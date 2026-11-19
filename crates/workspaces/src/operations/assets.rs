use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{WorkspaceStore, asset_root};

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

pub(super) fn definitions() -> [Value; 4] {
    let id = json!({"type": "integer", "minimum": 1});
    [
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
    ]
}

pub(super) fn call(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
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
        _ => return None,
    })
}
