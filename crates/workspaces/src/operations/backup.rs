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
        json!({"name": "cancel_backup_import", "description": "Discard a staged backup so nothing is replaced at the next start.",
            "inputSchema": none, "annotations": {"idempotentHint": true}}),
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
        "get_backup_import_status" | "cancel_backup_import" => Err("This tool takes no arguments"),
        _ => return None,
    })
}
