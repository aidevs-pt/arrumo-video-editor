//! App-wide operations: full backups, privacy and notification choices and the plugin tour.
use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{BackupSummary, WorkspaceStore};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathInput {
    path: String,
}

fn absolute(arguments: &Map<String, Value>) -> Result<String, &'static str> {
    let input: PathInput = serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Expected an absolute local path")?;
    if Path::new(&input.path).is_absolute() {
        Ok(input.path)
    } else {
        Err("Expected an absolute local path")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TourInput {
    #[serde(deserialize_with = "bounded_ids")]
    plugin_ids: Vec<String>,
}

fn bounded_ids<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let ids = Vec::<String>::deserialize(deserializer)?;
    if ids.len() > 64 || ids.iter().any(|id| !(1..=128).contains(&id.len())) {
        return Err(serde::de::Error::custom("too many or invalid plugin ids"));
    }
    Ok(ids)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrivacyInput {
    usage_data: bool,
    crash_reports: bool,
}

pub(super) fn definitions() -> Vec<Value> {
    let path = json!({"type": "object", "properties": {"path": {"type": "string", "minLength": 1}},
        "required": ["path"], "additionalProperties": false});
    let none = json!({"type": "object", "additionalProperties": false});
    vec![
        json!({"name": "export_backup", "description": "Write a full backup (settings, workspaces, profiles, assets, animations, finished videos and installed plugins, without API keys) to a new ZIP at an absolute local path. Every file is listed with its SHA-256.",
            "inputSchema": path}),
        json!({"name": "verify_backup", "description": "Check a backup ZIP's manifest and every file's size and SHA-256 without changing anything.",
            "inputSchema": path, "annotations": {"readOnlyHint": true}}),
        json!({"name": "stage_backup_import", "description": "Verify a backup and stage it to replace ALL local data at the next app start. The replaced data is kept in a dated folder beside it. Cancel with cancel_backup_import.",
            "inputSchema": path, "annotations": {"destructiveHint": true}}),
        json!({"name": "get_backup_import_status", "description": "Whether a verified backup is staged to be restored at the next start.",
            "inputSchema": none, "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_privacy_settings", "description": "Read whether anonymous usage counts and crash reports are allowed, and whether this build can send them at all.",
            "inputSchema": none, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_privacy_settings", "description": "Allow or stop anonymous usage counts and crash reports.",
            "inputSchema": {"type": "object", "properties": {"usageData": {"type": "boolean"}, "crashReports": {"type": "boolean"}},
                "required": ["usageData", "crashReports"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "get_notification_settings", "description": "Read whether finished, failed and waiting-for-review videos also raise operating system notifications.",
            "inputSchema": none, "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_notification_settings", "description": "Turn operating system notifications for generation events on or off for the whole app.",
            "inputSchema": {"type": "object", "properties": {"system": {"type": "boolean"}}, "required": ["system"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "get_plugin_tour", "description": "List the plugin introductions the creator has not seen yet (pending), or every plugin's introduction (all).",
            "inputSchema": {"type": "object", "properties": {"all": {"type": "boolean"}}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "finish_plugin_tour", "description": "Record that the creator has seen these plugins' introductions.",
            "inputSchema": {"type": "object", "properties": {"pluginIds": {"type": "array", "maxItems": 64, "items": {"type": "string", "minLength": 1, "maxLength": 128}}},
                "required": ["pluginIds"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "cancel_backup_import", "description": "Discard a staged backup so nothing is replaced at the next start.",
            "inputSchema": none, "annotations": {"idempotentHint": true}}),
        json!({"name": "list_editing_guides", "description": "List Arrumo's editing guides: the craft notes each production step's agent follows (script, visuals, animation, cutting, storyboard, sound, portrait cuts, publishing text, review). Read one with get_editing_guide before doing that kind of work yourself.",
            "inputSchema": none, "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_editing_guide", "description": "Read one editing guide as Markdown.",
            "inputSchema": {"type": "object", "properties": {"id": {"enum": crate::EDITING_GUIDES.map(|guide| guide.id)}},
                "required": ["id"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "export_backup" => absolute(arguments).and_then(|path| {
            store
                .export_backup(database, Path::new(&path))
                .map(|summary| json!({"backup": summary}))
                .map_err(|_| "Could not export the backup. Choose a new destination with free space")
        }),
        "verify_backup" => absolute(arguments).and_then(|path| {
            crate::verify_backup(Path::new(&path))
                .map(|manifest| json!({"backup": BackupSummary::from(&manifest)}))
                .map_err(|_| "This is not an intact Arrumo backup")
        }),
        "stage_backup_import" => absolute(arguments).and_then(|path| {
            crate::stage_backup_import(database, Path::new(&path))
                .map(|summary| json!({"backup": summary, "appliesAtNextStart": true}))
                .map_err(|_| "Could not stage the backup. It may be damaged, not an Arrumo backup, or need a newer Arrumo")
        }),
        "get_backup_import_status" if arguments.is_empty() => {
            Ok(json!({"pending": crate::pending_backup_import(database)}))
        }
        "cancel_backup_import" if arguments.is_empty() => crate::cancel_backup_import(database)
            .map(|()| json!({"pending": false}))
            .map_err(|_| "Could not cancel the staged backup"),
        "get_plugin_tour" => {
            let all = match arguments.get("all") {
                None => false,
                Some(Value::Bool(all)) if arguments.len() == 1 => *all,
                _ => return Some(Err("Expected an optional all boolean")),
            };
            let stops = if all { store.plugin_tour(&[]) } else { store.pending_plugin_tour() };
            stops
                .map(|stops| json!({"plugins": stops}))
                .map_err(|_| "Could not read the plugin tour")
        }
        "finish_plugin_tour" => serde_json::from_value::<TourInput>(Value::Object(arguments.clone()))
            .map_err(|_| "Expected pluginIds")
            .and_then(|input| {
                store
                    .mark_plugins_introduced(&input.plugin_ids)
                    .map(|()| json!({"finished": input.plugin_ids.len()}))
                    .map_err(|_| "Could not save the plugin tour")
            }),
        "get_privacy_settings" if arguments.is_empty() => store
            .privacy_settings()
            .map(|settings| json!({"privacy": settings}))
            .map_err(|_| "Could not read privacy settings"),
        "set_privacy_settings" => serde_json::from_value::<PrivacyInput>(Value::Object(arguments.clone()))
            .map_err(|_| "Expected usageData and crashReports booleans")
            .and_then(|input| {
                store
                    .set_privacy_settings(input.usage_data, input.crash_reports)
                    .map(|settings| json!({"privacy": settings}))
                    .map_err(|_| "Could not save privacy settings")
            }),
        "get_notification_settings" if arguments.is_empty() => store
            .system_notifications()
            .map(|system| json!({"notifications": {"system": system}}))
            .map_err(|_| "Could not read notification settings"),
        "set_notification_settings" => match (arguments.len(), arguments.get("system").and_then(Value::as_bool)) {
            (1, Some(system)) => store
                .set_system_notifications(system)
                .map(|system| json!({"notifications": {"system": system}}))
                .map_err(|_| "Could not save notification settings"),
            _ => Err("Expected a system boolean"),
        },
        "get_backup_import_status" | "cancel_backup_import" | "get_privacy_settings" | "get_notification_settings" => {
            Err("This tool takes no arguments")
        }
        "list_editing_guides" if arguments.is_empty() => Ok(json!({"guides": crate::EDITING_GUIDES})),
        "list_editing_guides" => Err("This tool takes no arguments"),
        "get_editing_guide" => arguments
            .get("id")
            .and_then(Value::as_str)
            .filter(|_| arguments.len() == 1)
            .and_then(crate::editing_guide)
            .map(|guide| json!({"id": guide.id, "title": guide.title, "stage": guide.stage, "markdown": guide.body}))
            .ok_or("Use a guide id from list_editing_guides"),
        _ => return None,
    })
}
