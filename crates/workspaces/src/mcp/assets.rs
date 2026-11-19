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

pub(super) fn definitions() -> [Value; 2] {
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
        "import_workspace_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: ImportInput| {
                let source = Path::new(&args.source);
                if !source.is_absolute() {
                    return Err("Asset source must be an absolute local path");
                }
                store
                    .import_asset(&asset_root(database), args.workspace_id, source)
                    .map(|asset| json!({"asset": asset}))
                    .map_err(|_| "Could not import asset. Check the workspace and media file")
            }),
        _ => return None,
    })
}
