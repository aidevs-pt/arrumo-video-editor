use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::WorkspaceStore;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListLanguages {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRevision {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    revision: Option<i64>,
    language: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveRevision {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    expected_revision: i64,
    language: Option<String>,
    subtitles: String,
}

pub(super) fn definitions() -> [Value; 3] {
    let id = json!({"type": "integer", "minimum": 1});
    let revision = json!({"type": "integer", "minimum": 0});
    let language = json!({"type": "string", "minLength": 2, "maxLength": 63});
    [
        json!({"name": "list_completed_subtitle_languages",
            "description": "List additional subtitle language tags for a verified completed output.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id
            }, "required": ["workspaceId", "runId", "formatId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_completed_subtitles",
            "description": "Read SRT text from a verified completed output. Omit revision for the latest version or use zero for the original primary track. Set language to read an additional track.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "revision": revision, "language": language
            }, "required": ["workspaceId", "runId", "formatId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "revise_completed_subtitles",
            "description": "Save a new SRT revision on a completed output. Read the latest revision before saving. To create an additional language track, set language and use expectedRevision zero. The request must fit the transport's message size limit.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "expectedRevision": revision, "language": language,
                "subtitles": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "formatId", "expectedRevision", "subtitles"], "additionalProperties": false},
            "annotations": {"destructiveHint": false}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    fn parse<T: serde::de::DeserializeOwned>(
        arguments: &Map<String, Value>,
    ) -> Result<T, &'static str> {
        serde_json::from_value(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid completed subtitle arguments")
    }

    Some(match name {
        "list_completed_subtitle_languages" => parse(arguments).and_then(|args: ListLanguages| {
            store.completed_subtitle_languages(args.workspace_id, args.run_id, args.format_id)
                .map(|languages| json!(languages))
                .map_err(|_| "Could not list languages for the completed output")
        }),
        "get_completed_subtitles" => parse(arguments).and_then(|args: ReadRevision| {
            store.completed_subtitle_track(database, args.workspace_id, args.run_id, args.format_id, args.language.as_deref(), args.revision)
                .map(|revision| json!(revision))
                .map_err(|_| "Could not read subtitles. Check the completed output and revision")
        }),
        "revise_completed_subtitles" => parse(arguments).and_then(|args: SaveRevision| {
            store.revise_completed_subtitle_track(args.workspace_id, args.run_id, args.format_id, args.language.as_deref(), args.expected_revision, &args.subtitles)
                .map(|revision| json!({"revision": revision}))
                .map_err(|_| "Could not save subtitle revision. Check the completed output, current version, and SRT timing")
        }),
        _ => return None,
    })
}
