use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{OutputFormatSettings, SupplementalOutputs, WorkspaceStore};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceName {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextInput {
    workspace_id: i64,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateProfileInput {
    workspace_id: i64,
    name: String,
    #[serde(default)]
    guidance: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateProfileInput {
    workspace_id: i64,
    profile_id: i64,
    name: String,
    guidance: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormatInput {
    workspace_id: i64,
    format_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateFormatInput {
    workspace_id: i64,
    settings: OutputFormatSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateFormatInput {
    workspace_id: i64,
    format_id: i64,
    settings: OutputFormatSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeliveryInput {
    workspace_id: i64,
    format_id: i64,
    settings: SupplementalOutputs,
}

pub(super) fn definitions() -> [Value; 11] {
    let id = json!({"type": "integer", "minimum": 1});
    let name = json!({"type": "string", "minLength": 1});
    let color = json!({"type": ["string", "null"]});
    let workspace = json!({"type": "object", "properties": {"workspaceId": id},
        "required": ["workspaceId"], "additionalProperties": false});
    let format = json!({"type": "object", "properties": {"workspaceId": id, "formatId": id},
        "required": ["workspaceId", "formatId"], "additionalProperties": false});
    let dimensions = json!({"type": "object", "properties": {
        "name": name, "width": id, "height": id, "enabled": {"type": "boolean"}
    }, "required": ["name", "width", "height", "enabled"]});
    let count = json!({"type": "integer", "minimum": 0, "maximum": 65535});
    let reference_ids = json!({"type": "array", "items": id});
    let text = json!({"type": "string"});
    let supplemental = json!({"type": "object", "properties": {
        "thumbnails": count, "titles": count, "descriptions": count, "tags": {"type": "boolean"},
        "thumbnail_guidance": text, "thumbnail_reference_ids": reference_ids, "specifications": text,
        "portrait_derivatives": {"type": "object", "properties": {
            "count": count,
            "source": {"oneOf": [
                {"type": "object", "properties": {
                    "type": {"const": "manual"}, "guidance": text, "thumbnails_per_video": count,
                    "thumbnail_guidance": text, "thumbnail_reference_ids": reference_ids
                }, "required": ["type"], "additionalProperties": false},
                {"type": "object", "properties": {"type": {"const": "profile"}, "profile_id": id},
                    "required": ["type", "profile_id"], "additionalProperties": false}
            ]}
        }, "additionalProperties": false}
    }, "additionalProperties": false});
    [
        json!({"name": "create_workspace", "description": "Create a workspace with its default output formats.",
            "inputSchema": {"type": "object", "properties": {"name": name},
                "required": ["name"], "additionalProperties": false}}),
        json!({"name": "get_workspace_context", "description": "Read the workspace's saved Markdown context.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_workspace_context", "description": "Replace the workspace's saved Markdown context.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "markdown": {"type": "string"}},
                "required": ["workspaceId", "markdown"], "additionalProperties": false}}),
        json!({"name": "list_profiles", "description": "List the selected workspace's production profiles.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "create_profile", "description": "Create a production profile in the selected workspace.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "name": name, "guidance": {"type": "string"}, "color": color
            }, "required": ["workspaceId", "name"], "additionalProperties": false}}),
        json!({"name": "update_profile", "description": "Replace a profile's name, guidance, and color. Omit the color to clear it.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id, "name": name, "guidance": {"type": "string"}, "color": color
            }, "required": ["workspaceId", "profileId", "name", "guidance"], "additionalProperties": false}}),
        json!({"name": "list_output_formats", "description": "List workspace output formats and their dimensions.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "create_output_format", "description": "Add a workspace output format.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "settings": dimensions},
                "required": ["workspaceId", "settings"], "additionalProperties": false}}),
        json!({"name": "update_output_format", "description": "Replace a workspace output format's name, dimensions and enabled state.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "formatId": id, "settings": dimensions},
                "required": ["workspaceId", "formatId", "settings"], "additionalProperties": false}}),
        json!({"name": "get_format_delivery", "description": "Read required and supplemental outputs for a workspace format.",
            "inputSchema": format, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_format_delivery", "description": "Replace a workspace format's supplemental outputs. Omitted settings reset to defaults.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "formatId": id, "settings": supplemental},
                "required": ["workspaceId", "formatId", "settings"], "additionalProperties": false}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "create_workspace" => parse(arguments).and_then(|args: WorkspaceName| {
            store
                .create(&args.name)
                .map(|workspace| json!({"workspace": workspace}))
                .map_err(|_| "Could not create workspace")
        }),
        "get_workspace_context" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .workspace_context(args.workspace_id)
                .map(|context| json!({"context": context}))
                .map_err(|_| "Could not read workspace context")
        }),
        "set_workspace_context" => parse(arguments).and_then(|args: ContextInput| {
            store
                .set_workspace_context(args.workspace_id, &args.markdown)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save workspace context")
        }),
        "list_profiles" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .list_profiles(args.workspace_id)
                .map(|profiles| json!({"profiles": profiles}))
                .map_err(|_| "Could not list workspace profiles")
        }),
        "create_profile" => parse(arguments).and_then(|args: CreateProfileInput| {
            store
                .create_profile(
                    args.workspace_id,
                    &args.name,
                    &args.guidance,
                    args.color.as_deref(),
                )
                .map(|profile| json!({"profile": profile}))
                .map_err(|_| "Could not create profile. Check the workspace, name and color")
        }),
        "update_profile" => parse(arguments).and_then(|args: UpdateProfileInput| {
            store
                .update_profile(
                    args.workspace_id,
                    args.profile_id,
                    &args.name,
                    &args.guidance,
                    args.color.as_deref(),
                )
                .map(|profile| json!({"profile": profile}))
                .map_err(
                    |_| "Could not update profile. Check the workspace, profile, name and color",
                )
        }),
        "list_output_formats" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .list_output_formats(args.workspace_id)
                .map(|formats| json!({"formats": formats}))
                .map_err(|_| "Could not list output formats")
        }),
        "create_output_format" => parse(arguments).and_then(|args: CreateFormatInput| {
            store
                .create_output_format(args.workspace_id, &args.settings)
                .map(|format| json!({"format": format}))
                .map_err(|_| "Could not create output format")
        }),
        "update_output_format" => parse(arguments).and_then(|args: UpdateFormatInput| {
            store
                .update_output_format(args.workspace_id, args.format_id, &args.settings)
                .map(|format| json!({"format": format}))
                .map_err(|_| "Could not update output format")
        }),
        "get_format_delivery" => parse(arguments).and_then(|args: FormatInput| {
            store
                .format_delivery(args.workspace_id, args.format_id)
                .map(|delivery| json!({"delivery": delivery}))
                .map_err(|_| "Could not read format delivery settings")
        }),
        "set_format_delivery" => parse(arguments).and_then(|args: DeliveryInput| {
            store
                .set_format_delivery(args.workspace_id, args.format_id, &args.settings)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save format delivery settings")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid workspace setup arguments")
}
