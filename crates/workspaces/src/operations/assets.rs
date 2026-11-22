use std::{borrow::Cow, path::Path};

use serde::Deserialize;

use serde_json::{Map, Value, json};

use crate::{AssetOrganization, BrandAssetRole, ContextImport, WorkspaceStore, asset_root};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextImportInput {
    workspace_id: i64,
    #[serde(default)]
    colors: Vec<String>,
    #[serde(default)]
    images: Vec<crate::context_research::SuggestedImage>,
    #[serde(default)]
    profiles: Vec<crate::context_research::SuggestedProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateVideoInput {
    workspace_id: i64,
    prompt: String,
    aspect_ratio: String,
    #[serde(default)]
    duration_seconds: Option<u32>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    references: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateAudioInput {
    workspace_id: i64,
    kind: crate::AudioKind,
    prompt: String,
    seconds: f64,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateSpeechInput {
    workspace_id: i64,
    text: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateImagesInput {
    workspace_id: i64,
    prompt: String,
    aspect_ratio: String,
    count: usize,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    references: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunInput {
    workspace_id: i64,
    run_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderInput {
    workspace_id: i64,
    provider: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DubAssetInput {
    workspace_id: i64,
    asset_id: i64,
    language: String,
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransformAudioInput {
    workspace_id: i64,
    asset_id: i64,
    transform: crate::AudioTransform,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    voice: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrandAssetInput {
    workspace_id: i64,
    asset_id: i64,
    role: BrandAssetRole,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchNotesInput {
    workspace_id: i64,
    asset_ids: Vec<i64>,
    notes: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetInput {
    workspace_id: i64,
    asset_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OrganizationInput {
    workspace_id: i64,
    asset_id: i64,
    settings: AssetOrganization,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveAssetInput {
    workspace_id: i64,
    asset_id: i64,
    confirmed_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplaceAssetInput {
    workspace_id: i64,
    asset_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttachCompletedMedia {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    asset_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetachCompletedMedia {
    workspace_id: i64,
    run_id: i64,
    attachment_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportCompletedVideoVariant {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameStripInput {
    workspace_id: i64,
    asset_id: Option<i64>,
    run_id: Option<i64>,
    format_id: Option<i64>,
    preview_attempt: Option<i64>,
    start: f64,
    end: f64,
    #[serde(default = "default_frames")]
    frames: u32,
    destination: String,
}

fn default_frames() -> u32 {
    8
}

pub(super) fn definitions() -> [Value; 32] {
    let id = json!({"type": "integer", "minimum": 1});
    let assignment = json!({"type": "object", "properties": {
        "workspaceId": id, "assetId": id, "role": {"enum": ["identity", "style"]}
    }, "required": ["workspaceId", "assetId", "role"], "additionalProperties": false});
    [
        json!({"name": "generate_library_images",
            "description": "Generate one to four images from a description with the workspace's image providers (in their order, or one named provider) and add them to the workspace assets with the given notes. model picks one of the named provider's models from list_image_models (choose by its bestFor line); without it the provider's selected model is used. references are workspace image asset IDs to follow; each result's references count says how many the provider accepted (0 when it fell back to the description alone).",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "prompt": {"type": "string", "minLength": 1, "maxLength": 4000},
                "aspectRatio": {"enum": crate::ASSET_ASPECT_RATIOS},
                "count": {"type": "integer", "minimum": 1, "maximum": 4},
                "notes": {"type": "string", "maxLength": 8000},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64},
                "model": {"type": "string", "minLength": 1, "maxLength": 128},
                "references": {"type": "array", "items": id, "maxItems": 9, "uniqueItems": true}
            }, "required": ["workspaceId", "prompt", "aspectRatio", "count"], "additionalProperties": false}}),
        json!({"name": "list_image_models",
            "description": "List the named models of each image provider on for this workspace, in provider order: label, what each is best for (per app language), whether it is selected by default, how many reference images it takes and its aspect ratios. Use it to choose provider and model for generate_library_images.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "generate_library_speech",
            "description": "Voice text with the workspace's first speech provider (or one named provider) and its configured voice, and add the audio to the workspace assets with the given notes. model picks one of the named provider's speech models from list_speech_models (choose by its bestFor line and character limit).",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "text": {"type": "string", "minLength": 1, "maxLength": 5000},
                "notes": {"type": "string", "maxLength": 8000},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64},
                "model": {"type": "string", "minLength": 1, "maxLength": 64}
            }, "required": ["workspaceId", "text"], "additionalProperties": false}}),
        json!({"name": "transform_audio_asset",
            "description": "Send the sound of a workspace audio or video asset to a provider and add the result as a new audio asset whose notes name the source. voice-change says the same words in another voice (voice overrides the provider's configured voice ID); voice-isolation keeps the voice and removes background noise and music. list_audio_transforms shows who offers each and the longest input they take.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "assetId": {"type": "integer", "minimum": 1},
                "transform": {"enum": ["voice-change", "voice-isolation"]},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64},
                "voice": {"type": "string", "minLength": 1, "maxLength": 256}
            }, "required": ["workspaceId", "assetId", "transform"], "additionalProperties": false}}),
        json!({"name": "list_audio_transforms",
            "description": "List which providers on for this workspace change or isolate voices, and the shortest and longest sound in seconds each takes.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "dub_asset",
            "description": "Dub the speech of a workspace audio or video asset into another language with a dubbing provider, wait for the provider (it can take minutes) and add the dub as a new audio asset labelled Dub whose notes name the source. language is a code from list_dubbing_providers.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "assetId": {"type": "integer", "minimum": 1},
                "language": {"type": "string", "minLength": 2, "maxLength": 12},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64}
            }, "required": ["workspaceId", "assetId", "language"], "additionalProperties": false}}),
        json!({"name": "list_dubbing_providers",
            "description": "List the dubbing providers on for this workspace, the language codes each dubs into and the longest sound in seconds each takes.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_speech_models",
            "description": "List the named models of each speech provider on for this workspace, in provider order: label, what each is best for (per app language), whether it is selected by default and how many characters it voices per request. Use it to choose provider and model for generate_library_speech.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "generate_library_audio",
            "description": "Make instrumental music or a sound effect from a written description with the workspace's first provider for that kind (or one named provider), and add it to the workspace assets with the given notes, labelled Music or Sound effect. The provider sets the length limits; a length outside them fails with the allowed range. Music in an approved asset plan can be laid under a video.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "kind": {"enum": ["music", "sound-effect"]},
                "prompt": {"type": "string", "minLength": 1, "maxLength": 10000},
                "seconds": {"type": "number", "exclusiveMinimum": 0, "maximum": 3600},
                "notes": {"type": "string", "maxLength": 8000},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64}
            }, "required": ["workspaceId", "kind", "prompt", "seconds"], "additionalProperties": false}}),
        json!({"name": "generate_library_video",
            "description": "Generate a short clip with the workspace's first video provider (or one named provider) and add it to the workspace assets with the given notes. A provider may offer several models (see its plugin settings); name one with provider and model. references are workspace image or MP4 video asset IDs to animate or follow (up to 9 images and 3 videos, as the model allows); the provider picks the matching endpoint and fails with the models that fit when the chosen one can't take them. Takes minutes; the model's duration limits apply.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "prompt": {"type": "string", "minLength": 1, "maxLength": 2500},
                "aspectRatio": {"type": "string", "enum": ["16:9", "9:16", "1:1"]},
                "durationSeconds": {"type": "integer", "minimum": 1, "maximum": 60},
                "notes": {"type": "string", "maxLength": 8000},
                "provider": {"type": "string", "minLength": 1, "maxLength": 64},
                "model": {"type": "string", "minLength": 1, "maxLength": 64},
                "references": {"type": "array", "items": id, "maxItems": 12, "uniqueItems": true}
            }, "required": ["workspaceId", "prompt", "aspectRatio"], "additionalProperties": false}}),
        json!({"name": "list_video_models",
            "description": "List the models of each video provider on for this workspace, in provider order: label, what each is best for (per app language), which one is selected by default, and for each way of calling it the reference images and videos it takes (inclusive [min, max]; with two images the second is the closing frame where a variant allows it), its length limits (allowedSeconds lists the only lengths it takes; 0 to 0 means the clip follows the reference video) and aspect ratios. Use it to choose provider, model and references for generate_library_video.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "detach_completed_media",
            "description": "Remove a post-completion attachment from a verified output while retaining the managed asset.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attachmentId": id
            }, "required": ["workspaceId", "runId", "attachmentId"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "import_completed_video_variant",
            "description": "Import a local MP4 as an alternate video for a verified completed output. The host verifies video dimensions, duration and audio before committing the asset and attachment.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "formatId", "source"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "attach_completed_media",
            "description": "Attach an existing managed asset to one verified output. Repeating a successful attachment returns added=false.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "assetId": id
            }, "required": ["workspaceId", "runId", "formatId", "assetId"], "additionalProperties": false},
            "annotations": {"destructiveHint": false, "idempotentHint": true}}),
        json!({"name": "list_workspace_assets", "description": "List managed assets and their IDs in a workspace.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_workspace_footage", "description": "List the workspace's source footage: the video assets a generation can cut from.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_pending_reviews", "description": "List the generation runs in a workspace that wait for a person's review, with what each review is about.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_saved_run_motion", "description": "List which animations of a generation run are already saved to the motion library: the source result, the animation index and the profile it was shared with.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "runId": {"type": "integer", "minimum": 1}},
                "required": ["workspaceId", "runId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_speech_provider_status", "description": "Check a speech provider for this workspace: its audio format, character limit with the selected model, and where its key is set. Never returns the key.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "provider": {"type": "string", "minLength": 1, "maxLength": 64}},
                "required": ["workspaceId", "provider"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "import_workspace_asset", "description": "Copy an existing media file at an absolute path on the local machine into managed workspace storage.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "source"], "additionalProperties": false}}),
        json!({"name": "import_thumbnail_reference", "description": "Import a local image as a thumbnail reference in managed workspace storage.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "source"], "additionalProperties": false}}),
        json!({"name": "import_workspace_context", "description": "Add what draft_workspace_context found to the workspace: brand colours, images (downloaded from their public pages with their descriptions, groups and labels, as brand marks, thumbnail references or assets) and profiles. Pass only the suggestions to keep. Existing colours and profiles with the same name are skipped.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id,
                "colors": {"type": "array", "maxItems": 16, "items": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$"}},
                "images": {"type": "array", "maxItems": 40, "items": {"type": "object", "properties": {
                    "url": {"type": "string"}, "name": {"type": "string"}, "description": {"type": "string"},
                    "group": {"type": ["string", "null"]}, "labels": {"type": "array", "items": {"type": "string"}},
                    "purpose": {"enum": ["brand", "thumbnail_reference", "asset"]}
                }, "required": ["url", "name", "description", "group", "labels", "purpose"], "additionalProperties": false}},
                "profiles": {"type": "array", "maxItems": 12, "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "description": {"type": "string"}, "guidance": {"type": "string"},
                    "color": {"type": ["string", "null"]}
                }, "required": ["name", "description"], "additionalProperties": false}}
            }, "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"openWorldHint": true}}),
        json!({"name": "set_asset_notes", "description": "Replace the usage notes for a managed asset. Pass an empty string to clear them.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "notes": {"type": "string"}
            }, "required": ["workspaceId", "assetId", "notes"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "set_batch_asset_notes", "description": "Replace usage notes on multiple assets in one workspace. All changes are applied together. Pass empty notes to clear them.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetIds": {"type": "array", "items": id, "minItems": 1, "uniqueItems": true},
                "notes": {"type": "string"}
            }, "required": ["workspaceId", "assetIds", "notes"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "list_brand_assets", "description": "List image assets assigned to workspace identity and style.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "attach_brand_asset", "description": "Assign a managed workspace image to identity or style.",
            "inputSchema": assignment, "annotations": {"idempotentHint": true}}),
        json!({"name": "detach_brand_asset", "description": "Remove a brand role from an image while keeping the managed asset.",
            "inputSchema": assignment, "annotations": {"idempotentHint": true}}),
        json!({"name": "get_asset_organization", "description": "Read an asset's group and labels.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "assetId": id},
                "required": ["workspaceId", "assetId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_asset_organization", "description": "Replace an asset's group and labels. An empty settings object clears both.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "settings": {"type": "object", "properties": {
                    "group": {"type": ["string", "null"]}, "labels": {"type": "array", "items": {"type": "string"}}
                }, "additionalProperties": false}
            }, "required": ["workspaceId", "assetId", "settings"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "replace_workspace_asset", "description": "Replace an active workspace asset with a same-kind local file. Live references follow the new asset ID and captured run inputs keep the previous media.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "assetId", "source"], "additionalProperties": false},
            "annotations": {"destructiveHint": true}}),
        json!({"name": "remove_workspace_asset", "description": "Remove an unused workspace asset and its managed copy after confirming its exact filename.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "confirmedName": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "assetId", "confirmedName"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "render_frame_strip", "description": "Draw evenly spaced frames from a stretch of video above its sound's waveform into a new PNG at an absolute local path, to check a cut, join or overlay. Name a video asset with assetId, or a run's final video with runId and formatId (add previewAttempt for that attempt's preview). Returns each frame's time. Use it at decision points, not to scan a whole video.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "assetId": id, "runId": id, "formatId": id, "previewAttempt": id,
                "start": {"type": "number", "minimum": 0}, "end": {"type": "number", "exclusiveMinimum": 0, "description": "At most 120 seconds after start."},
                "frames": {"type": "integer", "minimum": 2, "maximum": 16, "default": 8},
                "destination": {"type": "string", "description": "Absolute path of a new .png file."}
            }, "required": ["workspaceId", "start", "end", "destination"], "additionalProperties": false},
            "annotations": {"destructiveHint": false}}),
    ]
}

/// Generation calls, whose failures carry the host's own explanation (which model fits the
/// references, the provider's short reason) so agents can correct the call.
pub(super) fn call_generation(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, Cow<'static, str>>> {
    Some(match name {
        "generate_library_images" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid image generation arguments"))
            .and_then(|args: GenerateImagesInput| {
                store
                    .generate_library_images(
                        database,
                        args.workspace_id,
                        &args.prompt,
                        &args.aspect_ratio,
                        args.count,
                        &args.notes,
                        args.provider.as_deref(),
                        args.model.as_deref(),
                        &args.references,
                    )
                    .map(|images| json!({"assets": images.iter().map(|image| json!({"assetId": image.id, "provider": image.provider, "references": image.references})).collect::<Vec<_>>()}))
                    .map_err(|error| Cow::Owned(format!("Could not generate the images: {error}")))
            }),
        "generate_library_audio" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid audio arguments"))
            .and_then(|args: GenerateAudioInput| {
                store
                    .generate_library_audio(
                        database,
                        args.workspace_id,
                        args.kind,
                        &args.prompt,
                        args.seconds,
                        &args.notes,
                        args.provider.as_deref(),
                        None,
                    )
                    .map(|(id, provider)| json!({"assetId": id, "provider": provider}))
                    .map_err(|error| Cow::Owned(format!("Could not make the audio: {error}")))
            }),
        "dub_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid dubbing arguments"))
            .and_then(|args: DubAssetInput| {
                store
                    .dub_asset(database, args.workspace_id, args.asset_id, &args.language, args.provider.as_deref())
                    .map(|(id, provider)| json!({"assetId": id, "provider": provider}))
                    .map_err(|error| Cow::Owned(format!("Could not dub it: {error}")))
            }),
        "transform_audio_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid audio transform arguments"))
            .and_then(|args: TransformAudioInput| {
                store
                    .transform_audio_asset(database, args.workspace_id, args.asset_id, args.transform, args.provider.as_deref(), args.voice.as_deref())
                    .map(|(id, provider)| json!({"assetId": id, "provider": provider}))
                    .map_err(|error| Cow::Owned(format!("Could not process the sound: {error}")))
            }),
        "generate_library_speech" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid speech arguments"))
            .and_then(|args: GenerateSpeechInput| {
                store
                    .generate_library_speech(database, args.workspace_id, &args.text, &args.notes, args.provider.as_deref(), args.model.as_deref())
                    .map(|(id, provider)| json!({"assetId": id, "provider": provider}))
                    .map_err(|error| Cow::Owned(format!("Could not voice the text: {error}")))
            }),
        "generate_library_video" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| Cow::Borrowed("Invalid video arguments"))
            .and_then(|args: GenerateVideoInput| {
                store
                    .generate_library_video(database, args.workspace_id, &args.prompt, &args.aspect_ratio, args.duration_seconds, &args.notes, args.provider.as_deref(), args.model.as_deref(), &args.references)
                    .map(|(id, provider)| json!({"assetId": id, "provider": provider}))
                    .map_err(|error| Cow::Owned(format!("Could not generate the clip: {error}")))
            }),
        _ => return None,
    })
}

pub(super) fn call(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "render_frame_strip" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid frame strip arguments")
            .and_then(|args: FrameStripInput| {
                let source = match (args.asset_id, args.run_id, args.format_id) {
                    (Some(asset), None, None) if args.preview_attempt.is_none() => crate::FrameStripSource::Asset(asset),
                    (None, Some(run_id), Some(format_id)) => crate::FrameStripSource::Output {
                        run_id, format_id, preview_attempt: args.preview_attempt,
                    },
                    _ => return Err("Name either assetId, or runId with formatId"),
                };
                let video = store
                    .frame_strip_source(database, args.workspace_id, source)
                    .map_err(|_| "That video was not found in the workspace")?;
                crate::render_frame_strip(&video, args.start, args.end, args.frames, Path::new(&args.destination))
                    .map(|view| json!(view))
                    .map_err(|_| "Could not draw the frame strip. Use 2 to 16 frames, a range of up to 120 seconds inside the video, and a new absolute .png path")
            }),
        "list_audio_transforms" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid audio transform arguments")
            .and_then(|args: ListInput| {
                store
                    .audio_transform_providers(args.workspace_id)
                    .map(|providers| json!({"transforms": providers}))
                    .map_err(|_| "Could not list the audio transforms")
            }),
        "list_dubbing_providers" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid dubbing arguments")
            .and_then(|args: ListInput| {
                store
                    .dubbing_providers(args.workspace_id)
                    .map(|providers| json!({"providers": providers}))
                    .map_err(|_| "Could not list the dubbing providers")
            }),
        "list_speech_models" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid speech model arguments")
            .and_then(|args: ListInput| {
                store
                    .speech_models(args.workspace_id)
                    .map(|models| json!({"models": models}))
                    .map_err(|_| "Could not list the speech models")
            }),
        "list_image_models" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid image model arguments")
            .and_then(|args: ListInput| {
                store
                    .image_models(args.workspace_id)
                    .map(|models| json!({"models": models}))
                    .map_err(|_| "Could not list the image models")
            }),
        "list_video_models" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid video model arguments")
            .and_then(|args: ListInput| {
                store
                    .video_models(args.workspace_id)
                    .map(|models| json!({"models": models}))
                    .map_err(|_| "Could not list the video models")
            }),
        "detach_completed_media" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed media arguments")
            .and_then(|args: DetachCompletedMedia| {
                store.detach_completed_media(args.workspace_id, args.run_id, args.attachment_id)
                    .map(|removed| json!({"removed": removed}))
                    .map_err(|_| "Could not detach completed media")
            }),
        "import_completed_video_variant" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed video variant arguments")
            .and_then(|args: ImportCompletedVideoVariant| {
                let source = Path::new(&args.source);
                if !source.is_absolute() {
                    return Err("Variant source must be an absolute local path");
                }
                store.import_completed_video_variant(database, args.workspace_id, args.run_id, args.format_id, source)
                    .map(|asset| json!(asset))
                    .map_err(|_| "Could not import the MP4 variant for the verified output")
            }),
        "attach_completed_media" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed media arguments")
            .and_then(|args: AttachCompletedMedia| {
                store.attach_completed_media(database, args.workspace_id, args.run_id, args.format_id, args.asset_id)
                    .map(|added| json!({"added": added}))
                    .map_err(|_| "Could not attach media to the verified output")
            }),
        "list_workspace_assets" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: ListInput| {
                store
                    .list_assets(args.workspace_id)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not list workspace assets")
            }),
        "list_workspace_footage" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid footage arguments")
            .and_then(|args: ListInput| {
                store
                    .list_footage(args.workspace_id)
                    .map(|footage| json!({"footage": footage}))
                    .map_err(|_| "Could not list footage")
            }),
        "list_pending_reviews" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid review arguments")
            .and_then(|args: ListInput| {
                store
                    .pending_generation_reviews(args.workspace_id)
                    .map(|reviews| json!({"reviews": reviews}))
                    .map_err(|_| "Could not list pending reviews")
            }),
        "list_saved_run_motion" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid motion arguments")
            .and_then(|args: RunInput| {
                store
                    .saved_run_motion(args.workspace_id, args.run_id)
                    .map(|saved| json!({"saved": saved.into_iter().map(|(result, index, profile)| json!({"resultId": result, "motionIndex": index, "profileId": profile})).collect::<Vec<_>>()}))
                    .map_err(|_| "Could not list saved animations")
            }),
        "get_speech_provider_status" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid speech provider arguments")
            .and_then(|args: ProviderInput| {
                store
                    .speech_provider_status(args.workspace_id, &args.provider)
                    .map(|status| json!({"status": status}))
                    .map_err(|_| "Could not check the speech provider")
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
        "import_workspace_context" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid context import arguments")
            .and_then(|args: ContextImportInput| {
                store
                    .apply_context_import(
                        database,
                        args.workspace_id,
                        &ContextImport {
                            colors: args.colors,
                            images: args.images,
                            profiles: args.profiles,
                        },
                    )
                    .map(|outcome| json!(outcome))
                    .map_err(|_| "Could not import the context suggestions. Check the workspace and the counts")
            }),
        "set_asset_notes" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: NotesInput| {
                store
                    .set_asset_notes(args.workspace_id, args.asset_id, &args.notes)
                    .map(|asset| json!({"asset": asset}))
                    .map_err(|_| "Could not update usage notes for this asset")
            }),
        "set_batch_asset_notes" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid batch asset notes arguments")
            .and_then(|args: BatchNotesInput| {
                store
                    .set_batch_asset_notes(args.workspace_id, &args.asset_ids, &args.notes)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not save notes for all selected workspace assets")
            }),
        "list_brand_assets" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid workspace asset arguments")
            .and_then(|args: ListInput| {
                store
                    .list_brand_assets(args.workspace_id)
                    .map(|assets| json!({"assets": assets}))
                    .map_err(|_| "Could not list brand assets")
            }),
        "attach_brand_asset" | "detach_brand_asset" => {
            serde_json::from_value(Value::Object(arguments.clone()))
                .map_err(|_| "Invalid brand asset arguments")
                .and_then(|args: BrandAssetInput| {
                    if name == "attach_brand_asset" {
                        store
                            .attach_brand_asset(args.workspace_id, args.asset_id, args.role)
                            .map(|()| json!({"saved": true}))
                            .map_err(|_| "Could not assign brand role. Choose a workspace image")
                    } else {
                        store
                            .detach_brand_asset(args.workspace_id, args.asset_id, args.role)
                            .map(|removed| json!({"removed": removed}))
                            .map_err(|_| "Could not remove brand role")
                    }
                })
        }
        "get_asset_organization" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset organization arguments")
            .and_then(|args: AssetInput| {
                store
                    .asset_organization(args.workspace_id, args.asset_id)
                    .map(|settings| json!({"organization": settings}))
                    .map_err(|_| "Could not read asset organization")
            }),
        "set_asset_organization" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset organization arguments")
            .and_then(|args: OrganizationInput| {
                store
                    .set_asset_organization(args.workspace_id, args.asset_id, args.settings)
                    .map(|settings| json!({"organization": settings}))
                    .map_err(|_| "Could not save asset group and labels")
            }),
        "replace_workspace_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset replacement arguments")
            .and_then(|args: ReplaceAssetInput| {
                let source = Path::new(&args.source);
                if !source.is_absolute() {
                    return Err("Asset source must be an absolute local path");
                }
                store
                    .replace_asset(&asset_root(database), args.workspace_id, args.asset_id, source)
                    .map(|asset| json!({"asset": asset}))
                    .map_err(|_| "Could not replace asset. Check the workspace, active asset ID, and replacement media kind")
            }),
        "remove_workspace_asset" => serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid asset removal arguments")
            .and_then(|args: RemoveAssetInput| {
                store
                    .remove_workspace_asset(
                        database,
                        args.workspace_id,
                        args.asset_id,
                        &args.confirmed_name,
                    )
                    .map(|removed| json!({"removed": removed}))
                    .map_err(
                        |_| "Could not finish asset removal. Retry with the same IDs and filename",
                    )
            }),
        _ => return None,
    })
}
