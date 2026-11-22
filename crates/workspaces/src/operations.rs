use std::{borrow::Cow, path::Path};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::WorkspaceStore;

mod app_data;
mod assets;
mod generation;
mod jobs;
mod motion;
mod profiles;
mod providers;
mod settings;
mod setup;
mod subtitles;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActionArguments {
    plugin_id: String,
    action_id: String,
    package_sha256: String,
    workspace_id: Option<i64>,
    input: Value,
}

pub(crate) fn definitions() -> Vec<Value> {
    let no_input = json!({"type": "object", "additionalProperties": false});
    let mut tools = vec![
        json!({
            "name": "list_workspaces",
            "description": "List local Arrumo workspaces and their IDs.",
            "inputSchema": no_input,
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "list_plugin_actions",
            "description": "List installed plugin actions, their scopes and current package digests.",
            "inputSchema": no_input,
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "invoke_plugin_action",
            "description": "Invoke a declared plugin action in its app or workspace scope. Use the package digest returned by list_plugin_actions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pluginId": {"type": "string"},
                    "actionId": {"type": "string"},
                    "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "workspaceId": {"type": "integer", "minimum": 1, "description": "Required for workspace actions. Omit for app actions."},
                    "input": {}
                },
                "required": ["pluginId", "actionId", "packageSha256", "input"],
                "additionalProperties": false
            }
        }),
    ];
    tools.extend(generation::definitions());
    tools.extend(assets::definitions());
    tools.extend(jobs::definitions());
    tools.extend(motion::definitions());
    tools.extend(setup::definitions());
    tools.extend(profiles::definitions());
    tools.extend(providers::definitions());
    tools.extend(settings::definitions());
    tools.extend(subtitles::definitions());
    tools.extend(app_data::definitions());
    tools
}

pub(crate) fn invoke(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: Map<String, Value>,
) -> Option<Result<Value, Cow<'static, str>>> {
    if let Some(result) = assets::call_generation(store, database, name, &arguments) {
        return Some(result);
    }
    invoke_static(store, database, name, arguments).map(|result| result.map_err(Cow::Borrowed))
}

fn invoke_static(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "list_workspaces" if arguments.is_empty() => store
            .list()
            .map(|workspaces| json!({"workspaces": workspaces}))
            .map_err(|_| "Could not list workspaces"),
        "list_plugin_actions" if arguments.is_empty() => store
            .plugin_actions()
            .map(|actions| json!({"actions": actions}))
            .map_err(|_| "Could not list plugin actions"),
        "list_workspaces" | "list_plugin_actions" => Err("This tool takes no arguments"),
        "invoke_plugin_action" => {
            let arguments: ActionArguments = match serde_json::from_value(Value::Object(arguments))
            {
                Ok(arguments) => arguments,
                Err(_) => return Some(Err("Invalid plugin action arguments")),
            };
            if arguments.package_sha256.len() != 64 {
                return Some(Err("Use a package digest from list_plugin_actions"));
            }
            store.invoke_reviewed_plugin_action(
                &arguments.plugin_id, &arguments.action_id, &arguments.package_sha256,
                arguments.workspace_id, &arguments.input,
            ).map(|output| json!({"output": output}))
             .map_err(|_| "Plugin action could not run. Check the installed revision, scope and permissions")
        }
        _ => assets::call(store, database, name, &arguments)
            .or_else(|| jobs::call(store, name, &arguments))
            .or_else(|| motion::call(store, database, name, &arguments))
            .or_else(|| setup::call(store, name, &arguments))
            .or_else(|| profiles::call(store, name, &arguments))
            .or_else(|| providers::call(store, name, &arguments))
            .or_else(|| settings::call(store, name, &arguments))
            .or_else(|| subtitles::call(store, database, name, &arguments))
            .or_else(|| app_data::call(store, database, name, &arguments))
            .or_else(|| generation::call(store, database, name, arguments))?,
    })
}

/// The JSON schema of a caption style, or of changes to one: every field is optional.
pub(crate) fn caption_style_schema() -> serde_json::Value {
    let color = serde_json::json!({"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"});
    serde_json::json!({"type": "object", "description": "How burned-in subtitles look. A profile or a video sets only the fields it changes.",
        "properties": {
            "font": {"enum": ["sans", "serif", "mono"]},
            "bold": {"type": "boolean"},
            "size": {"type": "integer", "minimum": 20, "maximum": 120, "description": "Text height in tenths of a percent of the frame's shorter side; 55 by default."},
            "color": color, "outline_color": color,
            "outline": {"type": "integer", "minimum": 0, "maximum": 30, "description": "Outline width as a percent of the text height."},
            "background": {"type": "boolean", "description": "A box in the outline colour behind the text instead of an outline."},
            "position": {"enum": ["bottom", "middle", "top"]},
            "margin": {"type": ["integer", "null"], "minimum": 0, "maximum": 40, "description": "Percent of the frame height from the top or bottom edge; null for the automatic margin."}
        }, "additionalProperties": false})
}
