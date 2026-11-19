use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{
    GenerationRun, ProductionStage, ReviewDecision, RunControl, RunOptions, WorkspaceStore,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnqueueInput {
    workspace_id: i64,
    draft_id: i64,
    request_key: String,
    #[serde(default)]
    options: RunOptions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunInput {
    workspace_id: i64,
    run_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlInput {
    workspace_id: i64,
    run_id: i64,
    action: RunControl,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewInput {
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    decision: ReviewDecision,
    #[serde(default)]
    notes: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointInput {
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
    checkpoint: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageResultInput {
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
    stage: ProductionStage,
    result: Value,
}

pub(super) fn definitions() -> [Value; 11] {
    let id = json!({"type": "integer", "minimum": 1});
    let workspace = json!({"type": "object", "properties": {"workspaceId": id},
        "required": ["workspaceId"], "additionalProperties": false});
    let run = json!({"type": "object", "properties": {"workspaceId": id, "runId": id},
        "required": ["workspaceId", "runId"], "additionalProperties": false});
    [
        json!({"name": "list_video_drafts", "description": "List drafts in the selected workspace.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_generation_runs", "description": "List runs and their current states in a workspace.",
            "inputSchema": workspace, "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_generation_run", "description": "Read one run, including its captured settings.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "get_production_progress", "description": "Read stage results and pending review for one run.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "enqueue_video_draft", "description": "Queue a saved draft. Reuse the request key to retry the same submission.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "draftId": id, "requestKey": {"type": "string", "minLength": 1, "maxLength": 128},
                "options": {"type": "object", "properties": {
                    "autopilot": {"type": "boolean"}, "platform": {"type": ["string", "null"]},
                    "provider_overrides": {"type": "object", "additionalProperties": {
                        "type": "array", "items": {"type": "string"}, "uniqueItems": true
                    }}
                }, "additionalProperties": false}
            }, "required": ["workspaceId", "draftId", "requestKey"], "additionalProperties": false}}),
        json!({"name": "control_generation", "description": "Pause, resume, retry, or cancel a generation run.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "action": {"enum": ["pause", "resume", "retry", "cancel"]}
            }, "required": ["workspaceId", "runId", "action"], "additionalProperties": false}}),
        json!({"name": "get_generation_activity", "description": "Read the transition history of a run.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "claim_generation", "description": "Claim the next available run in a workspace and receive its attempt number, lease expiry, checkpoint, and current step.",
            "inputSchema": workspace}),
        json!({"name": "checkpoint_generation", "description": "Save progress and renew the lease for the current attempt.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attempt": id, "checkpoint": {}
            }, "required": ["workspaceId", "runId", "attempt", "checkpoint"], "additionalProperties": false}}),
        json!({"name": "submit_production_result", "description": "Submit the result for the current production stage and leased attempt. Delivery verification is separate.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attempt": id,
                "stage": {"enum": ["script", "assets", "preview", "supplemental_outputs", "final_render"]}, "result": {"type": "object"}
            }, "required": ["workspaceId", "runId", "attempt", "stage", "result"], "additionalProperties": false}}),
        json!({"name": "review_production_result", "description": "Submit an explicit approval, rejection, or revision request for a pending result.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id,
                "decision": {"enum": ["approved", "rejected", "revise"]}, "notes": {"type": "string"}
            }, "required": ["workspaceId", "runId", "resultId", "decision"], "additionalProperties": false}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: serde_json::Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    let arguments = Value::Object(arguments);
    Some(match name {
        "list_video_drafts" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .list_video_drafts(args.workspace_id)
                .map(|drafts| json!({"drafts": drafts}))
                .map_err(|_| "Could not list drafts")
        }),
        "list_generation_runs" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .list_generation_runs(args.workspace_id)
                .map(|runs| {
                    let summaries: Vec<_> = runs.iter().map(run_summary).collect();
                    json!({"runs": summaries})
                })
                .map_err(|_| "Could not list generation runs")
        }),
        "get_generation_run" => parse(arguments).and_then(|args: RunInput| {
            store
                .generation_run(args.workspace_id, args.run_id)
                .map_err(|_| "Could not read generation run")?
                .map(|run| json!({"run": run}))
                .ok_or("Generation run not found")
        }),
        "get_production_progress" => parse(arguments).and_then(|args: RunInput| {
            store
                .production_progress(args.workspace_id, args.run_id)
                .map(|progress| json!({"progress": progress}))
                .map_err(|_| "Could not read production progress")
        }),
        "enqueue_video_draft" => parse(arguments).and_then(|args: EnqueueInput| {
            store
                .enqueue_video_draft(
                    args.workspace_id,
                    args.draft_id,
                    &args.request_key,
                    &args.options,
                )
                .map(|run| json!({"run": run_summary(&run)}))
                .map_err(|_| "Could not enqueue draft. Check scope, request key and options")
        }),
        "control_generation" => parse(arguments).and_then(|args: ControlInput| {
            store
                .control_generation(args.workspace_id, args.run_id, args.action)
                .map(|run| json!({"run": run_summary(&run)}))
                .map_err(|_| "Could not change this run in its current state or workspace")
        }),
        "get_generation_activity" => parse(arguments).and_then(|args: RunInput| {
            store
                .generation_activity(args.workspace_id, args.run_id)
                .map(|events| json!({"events": events}))
                .map_err(|_| "Could not read generation activity")
        }),
        "claim_generation" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .claim_generation(args.workspace_id)
                .map(|claim| json!({"claim": claim}))
                .map_err(|_| "Could not claim generation")
        }),
        "checkpoint_generation" => parse(arguments).and_then(|args: CheckpointInput| {
            store
                .checkpoint_generation(
                    args.workspace_id,
                    args.run_id,
                    args.attempt,
                    &args.checkpoint,
                )
                .map(|lease_until| json!({"leaseUntil": lease_until}))
                .map_err(|_| "Could not save checkpoint. Check the workspace and current lease")
        }),
        "submit_production_result" => parse(arguments).and_then(|args: StageResultInput| {
            store
                .submit_production_result(
                    args.workspace_id,
                    args.run_id,
                    args.attempt,
                    args.stage,
                    &args.result,
                )
                .map(|id| json!({"resultId": id}))
                .map_err(
                    |_| "Could not submit result. Check the workspace, current lease and stage",
                )
        }),
        "review_production_result" => parse(arguments).and_then(|args: ReviewInput| {
            store
                .review_production_result(
                    args.workspace_id,
                    args.run_id,
                    args.result_id,
                    args.decision,
                    &args.notes,
                )
                .map(|()| json!({"submitted": true}))
                .map_err(|_| "Could not review this result. Check the workspace and pending review")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, &'static str> {
    serde_json::from_value(arguments).map_err(|_| "Invalid generation tool arguments")
}

fn run_summary(run: &GenerationRun) -> Value {
    json!({
        "id": run.id,
        "draftId": run.draft_id,
        "state": run.state,
        "title": run.display_title.as_deref().or_else(|| run.snapshot["draft"]["title"].as_str()),
        "pendingReviewId": run.pending_review_id,
    })
}
