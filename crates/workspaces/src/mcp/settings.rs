use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::WorkspaceStore;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginInput {
    plugin_id: String,
    workspace_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingInput {
    plugin_id: String,
    workspace_id: Option<i64>,
    setting_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueInput {
    plugin_id: String,
    workspace_id: Option<i64>,
    setting_id: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewInput {
    max_edge: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextGrantInput {
    workspace_id: i64,
    plugin_id: String,
    package_sha256: String,
    allowed: bool,
}

pub(super) fn definitions() -> [Value; 10] {
    let workspace_id = json!({"type": "integer", "minimum": 1});
    let workspace = json!({"type": "integer", "minimum": 1, "description": "Omit for app scope."});
    let id = json!({"type": "string", "minLength": 1});
    [
        json!({"name": "get_plugin_settings", "description": "Read non-secret settings and their effective sources for a plugin.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace},
                "required": ["pluginId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_plugin_setting", "description": "Save a non-secret setting using the value type declared by the plugin.",
            "inputSchema": {"type": "object", "properties": {
                "pluginId": id, "workspaceId": workspace, "settingId": id,
                "value": {"type": ["boolean", "integer", "string"]}
            }, "required": ["pluginId", "settingId", "value"], "additionalProperties": false}}),
        json!({"name": "clear_plugin_setting", "description": "Clear a non-secret setting at this scope so it can inherit.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false}}),
        json!({"name": "get_preview_settings", "description": "Read the app's recommended, overridden, and effective preview resolution limits.",
            "inputSchema": {"type": "object", "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_preview_max_edge", "description": "Set the app-wide preview long edge in pixels. Pass null to restore the recommendation.",
            "inputSchema": {"type": "object", "properties": {
                "maxEdge": {"type": ["integer", "null"], "minimum": 144, "maximum": 2160}
            }, "required": ["maxEdge"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "list_plugin_catalog", "description": "List bundled and installed plugins with declared capabilities, settings metadata, and package information.",
            "inputSchema": {"type": "object", "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_plugin_credential_status", "description": "Check whether a plugin credential is configured and where it comes from. Returns status only.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "clear_plugin_credential", "description": "Remove a plugin credential at the specified scope. A workspace can then inherit its app credential. Omit workspaceId to remove the app credential.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false},
            "annotations": {"idempotentHint": true, "destructiveHint": true}}),
        json!({"name": "list_plugin_context_requests", "description": "List installed packages requesting workspace context and their current grant status.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": workspace_id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "apply_reviewed_plugin_context_grant", "description": "Grant or revoke workspace context access after reviewing the specific plugin package. Use the digest from list_plugin_context_requests.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": workspace_id, "pluginId": id,
                "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                "allowed": {"type": "boolean"}
            }, "required": ["workspaceId", "pluginId", "packageSha256", "allowed"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "get_plugin_settings" => parse(arguments).and_then(|args: PluginInput| {
            store
                .plugin_settings(&args.plugin_id, args.workspace_id)
                .map(|settings| json!({"settings": settings}))
                .map_err(|_| "Could not read plugin settings")
        }),
        "set_plugin_setting" => parse(arguments).and_then(|args: ValueInput| {
            store
                .set_plugin_setting(&args.plugin_id, &args.setting_id, args.workspace_id, Some(&args.value))
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save plugin setting. Check its declaration, scope and value type")
        }),
        "clear_plugin_setting" => parse(arguments).and_then(|args: SettingInput| {
            store
                .set_plugin_setting(&args.plugin_id, &args.setting_id, args.workspace_id, None)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not clear plugin setting")
        }),
        "get_preview_settings" if arguments.is_empty() => store
            .preview_settings()
            .map(|settings| json!({"settings": settings}))
            .map_err(|_| "Could not read preview settings"),
        "get_preview_settings" => Err("Unexpected preview settings arguments"),
        "set_preview_max_edge" => parse(arguments).and_then(|args: PreviewInput| {
            let max_edge = serde_json::from_value::<Option<u32>>(args.max_edge)
                .map_err(|_| "Expected an integer preview edge or null")?;
            store
                .set_preview_max_edge(max_edge)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save preview edge. Use 144 to 2160 pixels or null")
        }),
        "list_plugin_catalog" if arguments.is_empty() => store
            .plugin_catalog()
            .map(|plugins| json!({"plugins": plugins}))
            .map_err(|_| "Could not list plugin catalog"),
        "list_plugin_catalog" => Err("Unexpected plugin catalog arguments"),
        "get_plugin_credential_status" => parse(arguments).and_then(|args: SettingInput| {
            store
                .plugin_credential_status(&args.plugin_id, &args.setting_id, args.workspace_id)
                .map(|status| json!({"status": status}))
                .map_err(|_| "Could not check credential status")
        }),
        "clear_plugin_credential" => parse(arguments).and_then(|args: SettingInput| {
            store
                .set_plugin_credential(&args.plugin_id, &args.setting_id, args.workspace_id, None)
                .map(|()| json!({"cleared": true}))
                .map_err(|_| "Could not clear plugin credential. Check the setting, scope and credential store")
        }),
        "list_plugin_context_requests" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .plugin_context_requests(args.workspace_id)
                .map(|requests| json!({"requests": requests}))
                .map_err(|_| "Could not list workspace context requests")
        }),
        "apply_reviewed_plugin_context_grant" => parse(arguments).and_then(|args: ContextGrantInput| {
            store
                .apply_reviewed_plugin_context_grant(&args.plugin_id, args.workspace_id, &args.package_sha256, args.allowed)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not change context grant. Check workspace, package digest and requested permission")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid settings arguments")
}
