use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{
    DeliveryOverrides, DimensionOverrides, OutputFormatSettings, SupplementalOutputs,
    WorkspaceStore,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceName {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileDraftInput {
    workspace_id: i64,
    name: String,
    about: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceRename {
    workspace_id: i64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceAppearance {
    workspace_id: i64,
    color: String,
    icon: String,
    #[serde(default)]
    image_asset_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceInput {
    workspace_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextInput {
    workspace_id: i64,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchInput {
    workspace_id: i64,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    notes: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileInput {
    workspace_id: i64,
    profile_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileFormatInput {
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileOverridesInput<T> {
    workspace_id: i64,
    profile_id: i64,
    format_id: i64,
    overrides: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeedbackInput {
    workspace_id: i64,
    title: String,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateFeedbackInput {
    workspace_id: i64,
    feedback_id: i64,
    title: String,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeedbackIdInput {
    workspace_id: i64,
    feedback_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrandColorInput {
    workspace_id: i64,
    color: String,
    name: String,
    for_videos: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrandColorsInput {
    workspace_id: i64,
    colors: Vec<String>,
}

pub(super) fn definitions() -> [Value; 29] {
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
    let profile = json!({"type": "object", "properties": {"workspaceId": id, "profileId": id},
        "required": ["workspaceId", "profileId"], "additionalProperties": false});
    let profile_format = json!({"type": "object", "properties": {
        "workspaceId": id, "profileId": id, "formatId": id
    }, "required": ["workspaceId", "profileId", "formatId"], "additionalProperties": false});
    [
        json!({"name": "create_workspace", "description": "Create a workspace with its default output formats.",
            "inputSchema": {"type": "object", "properties": {"name": name},
                "required": ["name"], "additionalProperties": false}}),
        json!({"name": "rename_workspace", "description": "Rename a workspace.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "name": {"type": "string", "minLength": 1, "maxLength": 120}},
                "required": ["workspaceId", "name"], "additionalProperties": false}}),
        json!({"name": "set_workspace_appearance", "description": "Set how a workspace is shown in the switcher: a #RRGGBB color, a preset icon, and optionally one of its image assets, which replaces the icon.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id,
                "color": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"},
                "icon": {"enum": crate::WORKSPACE_ICONS},
                "imageAssetId": {"type": ["integer", "null"], "minimum": 1}},
                "required": ["workspaceId", "color", "icon"], "additionalProperties": false}}),
        json!({"name": "get_workspace_context", "description": "Read the workspace's saved Markdown context.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_workspace_context", "description": "Replace the workspace's saved Markdown context.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "markdown": {"type": "string"}},
                "required": ["workspaceId", "markdown"], "additionalProperties": false}}),
        json!({"name": "list_context_sources", "description": "List the links read for the workspace context, most recent first, with whether each could be read and when (Unix seconds). draft_workspace_context records them.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": {"type": "integer", "minimum": 1}}, "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "forget_context_source", "description": "Forget one remembered context link. Returns whether it was there.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": {"type": "integer", "minimum": 1}, "source": {"type": "string", "minLength": 1, "maxLength": 2048}}, "required": ["workspaceId", "source"], "additionalProperties": false},
            "annotations": {"idempotentHint": true, "destructiveHint": false}}),
        json!({"name": "draft_workspace_context", "description": "Read up to eight public web pages, such as the creator's website or channel, and let the workspace's agent draft its context and suggest profiles. Returns the draft without saving it.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id,
                "sources": {"type": "array", "maxItems": 8, "items": {"type": "string", "maxLength": 2048}},
                "notes": {"type": "string", "maxLength": 8000}},
                "required": ["workspaceId"], "additionalProperties": false}}),
        json!({"name": "draft_profile", "description": "Let the workspace's agent draft a profile's one-line description and editing guidance from its name and a few words about the videos. Returns the draft without saving it.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id,
                "name": {"type": "string", "minLength": 1, "maxLength": 120},
                "about": {"type": "string", "minLength": 1, "maxLength": 4000}},
                "required": ["workspaceId", "name", "about"], "additionalProperties": false}}),
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
        json!({"name": "remove_output_format", "description": "Remove a workspace output format and its overrides. Existing generation snapshots retain their captured formats.",
            "inputSchema": format,
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "get_format_delivery", "description": "Read required and supplemental outputs for a workspace format.",
            "inputSchema": format, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_format_delivery", "description": "Replace a workspace format's supplemental outputs. Omitted settings reset to defaults.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "formatId": id, "settings": supplemental},
                "required": ["workspaceId", "formatId", "settings"], "additionalProperties": false}}),
        json!({"name": "list_profile_formats", "description": "List enabled formats with inherited and overridden dimensions for a profile.",
            "inputSchema": profile, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_profile_format_overrides", "description": "Replace the profile's dimension overrides for one format. Omit a dimension to inherit it.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id, "formatId": id,
                "overrides": {"type": "object", "properties": {"width": id, "height": id}, "additionalProperties": false}
            }, "required": ["workspaceId", "profileId", "formatId", "overrides"], "additionalProperties": false}}),
        json!({"name": "get_profile_delivery", "description": "Read workspace defaults, profile overrides, and effective delivery settings.",
            "inputSchema": profile_format, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_profile_delivery", "description": "Replace the profile's supplemental delivery overrides. Omitted fields inherit workspace values.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id, "formatId": id, "overrides": supplemental
            }, "required": ["workspaceId", "profileId", "formatId", "overrides"], "additionalProperties": false}}),
        json!({"name": "list_workspace_feedback", "description": "List saved workspace feedback and guidance cards.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "add_workspace_feedback", "description": "Add a workspace feedback card with a title and Markdown guidance.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "title": name, "markdown": text
            }, "required": ["workspaceId", "title", "markdown"], "additionalProperties": false}}),
        json!({"name": "update_workspace_feedback", "description": "Replace a workspace feedback card's title and Markdown guidance.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "feedbackId": id, "title": name, "markdown": text
            }, "required": ["workspaceId", "feedbackId", "title", "markdown"], "additionalProperties": false}}),
        json!({"name": "remove_workspace_feedback", "description": "Remove a workspace feedback card so future runs no longer read it.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "feedbackId": id},
                "required": ["workspaceId", "feedbackId"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "get_brand_colors", "description": "Read the workspace's saved brand palette: every color, plus each color's name and whether videos use it.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_brand_colors", "description": "Replace the workspace brand palette with an ordered list of hex colors.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "colors": {"type": "array", "items": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"}}
            }, "required": ["workspaceId", "colors"], "additionalProperties": false}}),
        json!({"name": "set_brand_color_details", "description": "Name a color in the workspace brand palette and choose whether videos use it.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "color": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"},
                "name": {"type": "string", "maxLength": 40}, "forVideos": {"type": "boolean"}
            }, "required": ["workspaceId", "color", "name", "forVideos"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
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
        "rename_workspace" => parse(arguments).and_then(|args: WorkspaceRename| {
            store
                .rename_workspace(args.workspace_id, &args.name)
                .map(|workspace| json!({"workspace": workspace}))
                .map_err(|_| "Could not rename workspace")
        }),
        "set_workspace_appearance" => parse(arguments).and_then(|args: WorkspaceAppearance| {
            store
                .set_workspace_appearance(args.workspace_id, &args.color, &args.icon, args.image_asset_id)
                .map(|workspace| json!({"workspace": workspace}))
                .map_err(|_| "Could not set the workspace appearance")
        }),
        "draft_profile" => parse(arguments).and_then(|args: ProfileDraftInput| {
            store
                .draft_profile(args.workspace_id, &args.name, &args.about)
                .map(|draft| json!({"draft": draft}))
                .map_err(|_| "Could not draft the profile")
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
        "list_context_sources" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .context_sources(args.workspace_id)
                .map(|sources| json!({"sources": sources}))
                .map_err(|_| "Could not read the context links")
        }),
        "forget_context_source" => parse(arguments).and_then(|args: SourceInput| {
            store
                .forget_context_source(args.workspace_id, &args.source)
                .map(|forgotten| json!({"forgotten": forgotten}))
                .map_err(|_| "Could not forget the context link")
        }),
        "draft_workspace_context" => parse(arguments).and_then(|args: ResearchInput| {
            store
                .draft_workspace_context(args.workspace_id, &args.sources, &args.notes)
                .map(|draft| json!(draft))
                .map_err(|_| "Could not draft the workspace context. Check that the agent is installed and the links are public")
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
        "remove_output_format" => parse(arguments).and_then(|args: FormatInput| {
            store
                .remove_output_format(args.workspace_id, args.format_id)
                .map(|removed| json!({"removed": removed}))
                .map_err(|_| "Could not remove output format")
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
        "list_profile_formats" => parse(arguments).and_then(|args: ProfileInput| {
            store
                .list_profile_formats(args.workspace_id, args.profile_id)
                .map(|formats| json!({"formats": formats}))
                .map_err(|_| "Could not list profile formats")
        }),
        "set_profile_format_overrides" => {
            parse(arguments).and_then(|args: ProfileOverridesInput<DimensionOverrides>| {
                store
                    .set_profile_format_overrides(
                        args.workspace_id,
                        args.profile_id,
                        args.format_id,
                        &args.overrides,
                    )
                    .map(|()| json!({"saved": true}))
                    .map_err(|_| "Could not save dimension overrides")
            })
        }
        "get_profile_delivery" => parse(arguments).and_then(|args: ProfileFormatInput| {
            store
                .profile_delivery(args.workspace_id, args.profile_id, args.format_id)
                .map(|delivery| json!({"delivery": delivery}))
                .map_err(|_| "Could not read profile delivery settings")
        }),
        "set_profile_delivery" => {
            parse(arguments).and_then(|args: ProfileOverridesInput<DeliveryOverrides>| {
                store
                    .set_profile_delivery(
                        args.workspace_id,
                        args.profile_id,
                        args.format_id,
                        &args.overrides,
                    )
                    .map(|()| json!({"saved": true}))
                    .map_err(|_| "Could not save profile delivery settings")
            })
        }
        "list_workspace_feedback" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .list_workspace_feedback(args.workspace_id)
                .map(|feedback| json!({"feedback": feedback}))
                .map_err(|_| "Could not list workspace feedback")
        }),
        "add_workspace_feedback" => parse(arguments).and_then(|args: FeedbackInput| {
            store
                .add_workspace_feedback(args.workspace_id, &args.title, &args.markdown)
                .map(|card| json!({"card": card}))
                .map_err(|_| "Could not add workspace feedback")
        }),
        "update_workspace_feedback" => parse(arguments).and_then(|args: UpdateFeedbackInput| {
            store
                .update_workspace_feedback(
                    args.workspace_id,
                    args.feedback_id,
                    &args.title,
                    &args.markdown,
                )
                .map(|card| json!({"card": card}))
                .map_err(|_| "Could not update workspace feedback")
        }),
        "remove_workspace_feedback" => parse(arguments).and_then(|args: FeedbackIdInput| {
            store
                .remove_workspace_feedback(args.workspace_id, args.feedback_id)
                .map(|removed| json!({"removed": removed}))
                .map_err(|_| "Could not remove workspace feedback")
        }),
        "get_brand_colors" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .brand_palette(args.workspace_id)
                .map(|palette| {
                    let colors: Vec<&str> = palette.iter().map(|color| color.hex.as_str()).collect();
                    json!({"colors": colors, "palette": palette})
                })
                .map_err(|_| "Could not read brand colors")
        }),
        "set_brand_colors" => parse(arguments).and_then(|args: BrandColorsInput| {
            store
                .set_brand_colors(args.workspace_id, &args.colors)
                .map(|colors| json!({"colors": colors}))
                .map_err(|_| "Could not save brand colors")
        }),
        "set_brand_color_details" => parse(arguments).and_then(|args: BrandColorInput| {
            store
                .set_brand_color_details(args.workspace_id, &args.color, &args.name, args.for_videos)
                .map(|palette| json!({"palette": palette}))
                .map_err(|_| "Could not save the brand color")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid workspace setup arguments")
}
