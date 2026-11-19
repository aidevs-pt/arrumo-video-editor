use serde_json::{Value, json};

use crate::{GenerationRun, RunControl, WorkspaceStore};

pub(super) type TaskResult = Result<Value, (i64, &'static str)>;

const NOT_FOUND: (i64, &str) = (-32001, "Task not found");
const NOT_CANCELABLE: (i64, &str) = (-32002, "Task cannot be canceled");
const INTERNAL: (i64, &str) = (-32603, "Could not read task");

pub(super) fn task_for_run(store: &WorkspaceStore, workspace_id: i64, run_id: i64) -> TaskResult {
    let run = read_run(store, workspace_id, run_id)?;
    render(store, workspace_id, &run)
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
    render(store, workspace_id, &run)
}

fn parse_id(id: &str) -> Option<(i64, i64)> {
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

fn render(store: &WorkspaceStore, workspace_id: i64, run: &GenerationRun) -> TaskResult {
    let state = match run.state.as_str() {
        "queued" => "TASK_STATE_SUBMITTED",
        "running" => "TASK_STATE_WORKING",
        "paused" => "TASK_STATE_INPUT_REQUIRED",
        "completed" => "TASK_STATE_COMPLETED",
        "failed" => "TASK_STATE_FAILED",
        "cancelled" => "TASK_STATE_CANCELED",
        _ => return Err(INTERNAL),
    };
    let id = format!("generation-{workspace_id}-{}", run.id);
    let context_id = format!("workspace-{workspace_id}");
    let mut task = json!({
        "id": id,
        "contextId": context_id,
        "status": {
            "state": state,
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
    if run.state == "completed" {
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
