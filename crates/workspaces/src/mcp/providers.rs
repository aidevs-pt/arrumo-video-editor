use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{WorkspaceStore, agent_plugins};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentInput {
    workspace_id: Option<i64>,
    plugin_id: String,
}

pub(super) fn definitions() -> [Value; 8] {
    let workspace = json!({"type": "integer", "minimum": 1, "description": "Omit for app scope."});
    let capability = json!({"type": "string", "minLength": 1});
    let preference = json!({"type": "object", "properties": {"workspaceId": workspace, "capability": capability},
        "required": ["capability"], "additionalProperties": false});
    let scope = json!({"type": "object", "properties": {"workspaceId": workspace}, "additionalProperties": false});
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
        json!({"name": "list_agent_plugins", "description": "List bundled plugins eligible to provide the production agent.",
            "inputSchema": {"type": "object", "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_agent_settings", "description": "Read the effective production agent and where it was configured.",
            "inputSchema": scope, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_agent_provider", "description": "Select an eligible production agent plugin for this scope.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": workspace, "pluginId": {"type": "string", "minLength": 1}
            }, "required": ["pluginId"], "additionalProperties": false}}),
        json!({"name": "clear_agent_provider", "description": "Clear the agent selection at this scope. A workspace then inherits the app selection.",
            "inputSchema": scope}),
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
        "list_agent_plugins" if arguments.is_empty() => Ok(json!({"plugins": agent_plugins()})),
        "list_agent_plugins" => Err("Unexpected agent listing arguments"),
        "get_agent_settings" => parse(arguments).and_then(|args: Scope| {
            store.agent_settings(args.workspace_id)
                .map(|settings| json!({"settings": settings}))
                .map_err(|_| "Could not read agent settings")
        }),
        "set_agent_provider" => parse(arguments).and_then(|args: AgentInput| {
            store.set_agent_provider(args.workspace_id, Some(&args.plugin_id))
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not select agent plugin. Check the workspace and plugin ID")
        }),
        "clear_agent_provider" => parse(arguments).and_then(|args: Scope| {
            store.set_agent_provider(args.workspace_id, None)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not clear agent selection")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid provider preference arguments")
}
