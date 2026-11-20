use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{PluginActionJobRequest, WorkspaceStore};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobScope {
    id: String,
    workspace_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListScope {
    workspace_id: Option<i64>,
}

pub(super) fn definitions() -> [Value; 4] {
    let key = json!({"type": "string", "format": "uuid"});
    let space = json!({"type": "integer", "minimum": 1});
    let scope = json!({"type": "object", "properties": {
        "id": key, "workspaceId": space,
    }, "required": ["id"], "additionalProperties": false});
    [
        json!({"name": "list_plugin_action_jobs", "description": "List up to ten recent plugin jobs in one scope.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": space}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "queue_plugin_action_job",
            "description": "Persist a declared plugin action against a reviewed package digest. Reuse the same job UUID only for an identical request.",
            "inputSchema": {"type": "object", "properties": {
                "id": key, "workspaceId": space, "pluginId": {"type": "string"},
                "actionId": {"type": "string"}, "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                "input": {}
            }, "required": ["id", "pluginId", "actionId", "packageSha256", "input"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "get_plugin_action_job",
            "description": "Read durable state and result of a plugin action job in its original scope.",
            "inputSchema": scope, "annotations": {"readOnlyHint": true}}),
        json!({"name": "process_plugin_action_job",
            "description": "Claim and execute a queued plugin action or recover a job with an expired claim.",
            "inputSchema": scope, "annotations": {"destructiveHint": false}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "list_plugin_action_jobs" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid plugin action job scope")
            .and_then(|scope: ListScope| {
                store
                    .list_plugin_action_jobs(scope.workspace_id)
                    .map(|jobs| json!(jobs))
                    .map_err(|_| "Could not list plugin jobs")
            }),
        "queue_plugin_action_job" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid plugin action job request")
            .and_then(|request: PluginActionJobRequest| {
                store
                    .queue_plugin_action_job(&request)
                    .map(|job| json!(job))
                    .map_err(|_| "Could not queue the reviewed action job")
            }),
        "get_plugin_action_job" | "process_plugin_action_job" => {
            serde_json::from_value(Value::Object(arguments.clone()))
                .map_err(|_| "Invalid plugin action job scope")
                .and_then(|scope: JobScope| {
                    let id = scope
                        .id
                        .parse()
                        .map_err(|_| "Invalid plugin action job UUID")?;
                    let job = if name == "get_plugin_action_job" {
                        store.plugin_action_job(scope.workspace_id, id)
                    } else {
                        store.process_plugin_action_job(scope.workspace_id, id)
                    };
                    job.map(|job| json!(job))
                        .map_err(|_| "Plugin action job unavailable in this scope")
                })
        }
        _ => return None,
    })
}
