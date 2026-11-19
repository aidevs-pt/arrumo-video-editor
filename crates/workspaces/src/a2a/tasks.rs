use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{GenerationRun, RunControl, WorkspaceStore};

pub(super) type TaskResult = Result<Value, (i64, &'static str)>;

const NOT_FOUND: (i64, &str) = (-32001, "Task not found");
const NOT_CANCELABLE: (i64, &str) = (-32002, "Task cannot be canceled");
const INTERNAL: (i64, &str) = (-32603, "Could not read task");

pub(super) fn task_for_run(store: &WorkspaceStore, workspace_id: i64, run_id: i64) -> TaskResult {
    let run = read_run(store, workspace_id, run_id)?;
    render(store, workspace_id, &run, true)
}

pub(super) fn handle(store: &WorkspaceStore, method: &str, params: &Value) -> TaskResult {
    let (workspace_id, run_id) = params["id"].as_str().and_then(parse_id).ok_or(NOT_FOUND)?;
    let mut run = read_run(store, workspace_id, run_id)?;
    if method == "CancelTask" && run.state != "cancelled" {
        run = match store.control_generation(workspace_id, run_id, RunControl::Cancel) {
            Ok(run) => run,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let latest = read_run(store, workspace_id, run_id)?;
                if latest.state != "cancelled" {
                    return Err(NOT_CANCELABLE);
                }
                latest
            }
            Err(_) => return Err(INTERNAL),
        };
    }
    render(store, workspace_id, &run, true)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ListInput {
    context_id: Option<String>,
    status: Option<String>,
    page_size: Option<i64>,
    page_token: Option<String>,
    history_length: Option<i32>,
    status_timestamp_after: Option<String>,
    include_artifacts: bool,
}

pub(super) fn list(store: &WorkspaceStore, params: Value) -> TaskResult {
    const INVALID: (i64, &str) = (-32602, "Invalid task list parameters");
    let input: ListInput = serde_json::from_value(params).map_err(|_| INVALID)?;
    let page_size = input.page_size.unwrap_or(50);
    if !(1..=100).contains(&page_size) || input.history_length.is_some_and(|length| length < 0) {
        return Err(INVALID);
    }
    let workspace = input
        .context_id
        .as_deref()
        .map(|value| {
            let id: i64 = value
                .strip_prefix("workspace-")
                .ok_or(INVALID)?
                .parse()
                .map_err(|_| INVALID)?;
            if id <= 0 || value != format!("workspace-{id}") {
                return Err(INVALID);
            }
            Ok(id)
        })
        .transpose()?;
    let state = input
        .status
        .as_deref()
        .map(|value| {
            STATES
                .iter()
                .find(|(_, status)| *status == value)
                .map(|(state, _)| *state)
                .ok_or(INVALID)
        })
        .transpose()?;
    let offset = match input.page_token.as_deref() {
        None | Some("") => 0,
        Some(token) => token
            .strip_prefix("v1-")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0 && *value <= i64::MAX - page_size)
            .ok_or(INVALID)?,
    };
    let tx = store.0.unchecked_transaction().map_err(|_| INTERNAL)?;
    let after = if let Some(timestamp) = &input.status_timestamp_after {
        let parsed: Option<f64> = tx
            .query_row("SELECT julianday(?1)", [timestamp], |row| row.get(0))
            .map_err(|_| INTERNAL)?;
        Some(parsed.ok_or(INVALID)?)
    } else {
        None
    };
    let scope = "FROM generation_runs AS run
         JOIN generation_activity AS activity ON activity.id = (
             SELECT id FROM generation_activity WHERE run_id = run.id ORDER BY id DESC LIMIT 1
         )
         WHERE (?1 IS NULL OR run.workspace_id = ?1)
           AND (?2 IS NULL OR run.state = ?2)
           AND (?3 IS NULL OR julianday(activity.created_at, 'unixepoch') >= ?3)";
    let total: i64 = tx
        .query_row(
            &format!("SELECT count(*) {scope}"),
            params![workspace, state, after],
            |row| row.get(0),
        )
        .map_err(|_| INTERNAL)?;
    let mut statement = tx
        .prepare(&format!(
            "SELECT run.workspace_id, run.id {scope}
         ORDER BY activity.created_at DESC, activity.id DESC LIMIT ?4 OFFSET ?5"
        ))
        .map_err(|_| INTERNAL)?;
    let ids = statement
        .query_map(params![workspace, state, after, page_size, offset], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|_| INTERNAL)?;
    let tasks = ids
        .map(|row| {
            let (workspace_id, run_id) = row.map_err(|_| INTERNAL)?;
            let run = read_run(store, workspace_id, run_id)?;
            render(store, workspace_id, &run, input.include_artifacts)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next = offset + tasks.len() as i64;
    let token = if next < total {
        format!("v1-{next}")
    } else {
        String::new()
    };
    Ok(json!({"tasks": tasks, "nextPageToken": token, "pageSize": page_size, "totalSize": total}))
}

pub(super) fn parse_id(id: &str) -> Option<(i64, i64)> {
    let (workspace, run) = id.strip_prefix("generation-")?.split_once('-')?;
    let workspace: i64 = workspace.parse().ok()?;
    let run: i64 = run.parse().ok()?;
    (workspace > 0 && run > 0 && id == format!("generation-{workspace}-{run}"))
        .then_some((workspace, run))
}

fn read_run(
    store: &WorkspaceStore,
    workspace_id: i64,
    run_id: i64,
) -> Result<GenerationRun, (i64, &'static str)> {
    store
        .generation_run(workspace_id, run_id)
        .map_err(|_| INTERNAL)?
        .ok_or(NOT_FOUND)
}

const STATES: [(&str, &str); 6] = [
    ("queued", "TASK_STATE_SUBMITTED"),
    ("running", "TASK_STATE_WORKING"),
    ("paused", "TASK_STATE_INPUT_REQUIRED"),
    ("completed", "TASK_STATE_COMPLETED"),
    ("failed", "TASK_STATE_FAILED"),
    ("cancelled", "TASK_STATE_CANCELED"),
];

fn render(
    store: &WorkspaceStore,
    workspace_id: i64,
    run: &GenerationRun,
    include_artifacts: bool,
) -> TaskResult {
    let state = STATES
        .iter()
        .find(|(state, _)| *state == run.state)
        .ok_or(INTERNAL)?
        .1;
    let timestamp: String = store
        .0
        .query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch')
         FROM generation_activity WHERE run_id = ?1 ORDER BY id DESC LIMIT 1",
            [run.id],
            |row| row.get(0),
        )
        .map_err(|_| INTERNAL)?;
    let id = format!("generation-{workspace_id}-{}", run.id);
    let context_id = format!("workspace-{workspace_id}");
    let mut task = json!({
        "id": id,
        "contextId": context_id,
        "status": {
            "state": state,
            "timestamp": timestamp,
            "message": {
                "role": "ROLE_AGENT",
                "messageId": uuid::Uuid::new_v4().to_string(),
                "contextId": context_id,
                "taskId": id,
                "parts": [{"data": {"workspaceId": workspace_id, "runId": run.id,
                    "state": run.state, "pendingReviewId": run.pending_review_id}}]
            }
        }
    });
    if include_artifacts && run.state == "completed" {
        let delivery = store
            .verified_delivery(workspace_id, run.id)
            .map_err(|_| INTERNAL)?;
        task["artifacts"] = json!([{
            "artifactId": format!("{id}-delivery"),
            "name": "Verified delivery",
            "parts": [{"data": delivery}]
        }]);
    }
    Ok(task)
}
