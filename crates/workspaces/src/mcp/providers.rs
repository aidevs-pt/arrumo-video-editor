use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::WorkspaceStore;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope {
    workspace_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreferenceInput {
    workspace_id: Option<i64>,
    capability: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetInput {
    workspace_id: Option<i64>,
    capability: String,
    order: Vec<String>,
}

pub(super) fn definitions() -> [Value; 4] {
    let workspace = json!({"type": "integer", "minimum": 1, "description": "Omit for app scope."});
    let capability = json!({"type": "string", "minLength": 1});
    let preference = json!({"type": "object", "properties": {"workspaceId": workspace, "capability": capability},
        "required": ["capability"], "additionalProperties": false});
    [
        json!({"name": "list_provider_preferences", "description": "List provider capabilities with available providers, configured orders and effective sources.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": workspace}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_provider_preferences", "description": "Read provider ordering for one capability and scope.",
            "inputSchema": preference, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_provider_preferences", "description": "Set the provider order for a capability. An empty order disables providers in that scope.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": workspace, "capability": capability,
                "order": {"type": "array", "items": {"type": "string"}, "uniqueItems": true}
            }, "required": ["capability", "order"], "additionalProperties": false}}),
        json!({"name": "clear_provider_preferences", "description": "Remove the configured order to inherit the parent or bundled defaults.",
            "inputSchema": preference}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "list_provider_preferences" => parse(arguments).and_then(|args: Scope| {
            store
                .list_provider_preferences(args.workspace_id)
                .map(|preferences| json!({"preferences": preferences}))
                .map_err(|_| "Could not list provider preferences")
        }),
        "get_provider_preferences" => parse(arguments).and_then(|args: PreferenceInput| {
            store
                .provider_preferences(&args.capability, args.workspace_id)
                .map(|preferences| json!({"preferences": preferences}))
                .map_err(|_| "Could not read provider preferences")
        }),
        "set_provider_preferences" => parse(arguments).and_then(|args: SetInput| {
            store
                .set_provider_preferences(&args.capability, args.workspace_id, Some(&args.order))
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save provider order. Check the capability, scope and provider IDs")
        }),
        "clear_provider_preferences" => parse(arguments).and_then(|args: PreferenceInput| {
            store
                .set_provider_preferences(&args.capability, args.workspace_id, None)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not clear provider order")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid provider preference arguments")
}
