use std::path::Path;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{
    DraftSettings, GenerationRun, PreviewFormatDecision, ProductionStage, ReplayMode,
    ReviewDecision, RunControl, RunOptions, SupplementalReview, WorkspaceStore,
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
    save_motion_indexes: Option<Vec<usize>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewReviewInput {
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    decisions: Vec<PreviewFormatDecision>,
    #[serde(default)]
    notes: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SupplementalReviewInput {
    workspace_id: i64,
    run_id: i64,
    result_id: i64,
    review: SupplementalReview,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerifyDeliveryInput {
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportDeliveryInput {
    workspace_id: i64,
    run_id: i64,
    destination: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NotificationInput {
    workspace_id: i64,
    #[serde(default)]
    after_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplayInput {
    workspace_id: i64,
    run_id: i64,
    request_key: String,
    mode: ReplayMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FailureInput {
    workspace_id: i64,
    run_id: i64,
    attempt: i64,
    detail: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateDraftInput {
    workspace_id: i64,
    settings: DraftSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateDraftInput {
    workspace_id: i64,
    draft_id: i64,
    settings: DraftSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameCompletedInput {
    workspace_id: i64,
    run_id: i64,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompletedFeedbackInput {
    workspace_id: i64,
    run_id: i64,
    result_id: Option<i64>,
    markdown: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaStageInput {
    workspace_id: i64,
    run_id: i64,
    preview_edit: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubtitleInput {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateSubtitlesInput {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteDraftInput {
    workspace_id: i64,
    draft_id: i64,
    confirmed_title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteRunInput {
    workspace_id: i64,
    run_id: i64,
    confirmed_title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScheduleInput {
    workspace_id: i64,
    run_id: i64,
    format_id: i64,
    #[serde(deserialize_with = "Option::deserialize")]
    date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CalendarInput {
    workspace_id: i64,
    profile_id: Option<i64>,
    start: String,
    end: String,
}

pub(super) fn definitions() -> [Value; 38] {
    let id = json!({"type": "integer", "minimum": 1});
    let workspace = json!({"type": "object", "properties": {"workspaceId": id},
        "required": ["workspaceId"], "additionalProperties": false});
    let run = json!({"type": "object", "properties": {"workspaceId": id, "runId": id},
        "required": ["workspaceId", "runId"], "additionalProperties": false});
    let draft = json!({"type": "object", "properties": {
        "title": {"type": "string", "minLength": 1},
        "profile_id": {"type": ["integer", "null"], "minimum": 1},
        "intent": {"type": "string"},
        "source_ids": {"type": "array", "items": id, "uniqueItems": true}
    }, "required": ["title"], "additionalProperties": false});
    let date =
        json!({"type": "string", "format": "date", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"});
    [
        json!({"name": "create_video_draft", "description": "Save a draft with a brief, an ordered list of managed video sources, or both.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "settings": draft},
                "required": ["workspaceId", "settings"], "additionalProperties": false}}),
        json!({"name": "update_video_draft", "description": "Replace a saved draft's title, brief, profile, and ordered sources. Send the full draft settings.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "draftId": id, "settings": draft},
                "required": ["workspaceId", "draftId", "settings"], "additionalProperties": false}}),
        json!({"name": "delete_video_draft", "description": "Delete a draft that has never been queued. Confirm its exact saved title. Managed source assets remain in the library.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "draftId": id, "confirmedTitle": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "draftId", "confirmedTitle"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
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
                    "autopilot": {"type": "boolean"}, "generate_delivery_subtitles": {"type": "boolean", "default": false}, "save_motion_to_library": {"type": "boolean", "default": true}, "platform": {"type": ["string", "null"]},
                    "provider_overrides": {"type": "object", "additionalProperties": {
                        "type": "array", "items": {"type": "string"}, "uniqueItems": true
                    }}
                }, "additionalProperties": false}
            }, "required": ["workspaceId", "draftId", "requestKey"], "additionalProperties": false}}),
        json!({"name": "control_generation", "description": "Pause, resume, retry, or cancel a generation run.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "action": {"enum": ["pause", "resume", "retry", "cancel", "stop"]}
            }, "required": ["workspaceId", "runId", "action"], "additionalProperties": false}}),
        json!({"name": "process_script_stage", "description": "Run the run's agent on its queued Script stage and submit the draft for review. Earlier drafts' review notes are included. The run fails with a reason if the agent cannot finish.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "runId": id},
                "required": ["workspaceId", "runId"], "additionalProperties": false}}),
        json!({"name": "process_supplemental_writing", "description": "Let the run's agent write the titles, descriptions and tags each format asks for, from the approved script, and generate its thumbnails through the run's image providers. Stages that also need portrait cuts are left to renderers.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "runId": id},
                "required": ["workspaceId", "runId"], "additionalProperties": false}}),
        json!({"name": "process_preview_edit", "description": "Let the run's agent choose the cuts from the recorded footage, following the approved script and transcripts, and render the preview for review.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "runId": id},
                "required": ["workspaceId", "runId"], "additionalProperties": false}}),
        json!({"name": "process_asset_plan", "description": "Let the run's agent plan which library assets and animations support each script section. Applies when the run has no motion renderer.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": id, "runId": id},
                "required": ["workspaceId", "runId"], "additionalProperties": false}}),
        json!({"name": "process_media_stage", "description": "Claim and execute one ready media stage. Pass previewEdit (or an empty object for whole clips) for preview rendering. Omit it for final rendering or verification.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "previewEdit": {"type": "object"}
            }, "required": ["workspaceId", "runId"], "additionalProperties": false}}),
        json!({"name": "generate_delivery_subtitles", "description": "Send audio from one managed rendered output to a transcription provider selected in the run snapshot. Validate and publish a timed SRT only when no subtitle file exists.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id
            }, "required": ["workspaceId", "runId", "formatId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "openWorldHint": true}}),
        json!({"name": "import_delivery_subtitles", "description": "Validate and import a local SRT for a rendered output at the delivery verification stage.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "source": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "formatId", "source"], "additionalProperties": false}}),
        json!({"name": "verify_generation_delivery", "description": "Verify the final video and subtitle files in the managed delivery directory and complete the current leased run.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attempt": id
            }, "required": ["workspaceId", "runId", "attempt"], "additionalProperties": false}}),
        json!({"name": "export_generation_delivery", "description": "Write the completed run's verified video and subtitle files to a new ZIP at an absolute path on the local machine.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "destination": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "destination"], "additionalProperties": false}}),
        json!({"name": "rename_completed_video", "description": "Change the display title of a completed video.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "title": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "title"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "add_completed_video_feedback", "description": "Save Markdown feedback for the completed run or for one of its production results.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id, "markdown": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "markdown"], "additionalProperties": false}}),
        json!({"name": "recover_delivery_cleanup", "description": "Retry cleanup of managed delivery files for runs already deleted in this workspace.",
            "inputSchema": workspace, "annotations": {"idempotentHint": true}}),
        json!({"name": "set_output_schedule", "description": "Assign a calendar date to a verified output. Pass null to clear the date.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "formatId": id, "date": {"anyOf": [date, {"type": "null"}]}
            }, "required": ["workspaceId", "runId", "formatId", "date"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "list_scheduled_outputs", "description": "List scheduled outputs in an inclusive date range with optional profile filtering.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "profileId": id, "start": date, "end": date
            }, "required": ["workspaceId", "start", "end"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "delete_generation", "description": "Stop and delete a generation run and its managed files after confirming the exact display title. Runs referenced by replays are protected.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "confirmedTitle": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "confirmedTitle"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "delete_completed_video", "description": "Delete a completed run and its managed delivery files after confirming the exact display title. Runs referenced by replays are protected.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "confirmedTitle": {"type": "string", "minLength": 1}
            }, "required": ["workspaceId", "runId", "confirmedTitle"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
        json!({"name": "get_completed_video", "description": "Read the verified outputs and production history for a completed video.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "list_generation_notifications", "description": "Read up to 100 review and completion events in ascending order. Pass the last event ID to continue polling.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "afterId": {"type": "integer", "minimum": 0}
            }, "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "read_completion_notification", "description": "Mark a completed run's notification as read. Returns whether the read state changed.",
            "inputSchema": run, "annotations": {"idempotentHint": true, "destructiveHint": false}}),
        json!({"name": "get_generation_plan", "description": "Read the prepared production plan for a run.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "replay_generation", "description": "Queue a new run from a completed video using captured or current settings. Reuse the request key for the same submission.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "requestKey": {"type": "string", "minLength": 1, "maxLength": 128},
                "mode": {"enum": ["captured", "current"]}
            }, "required": ["workspaceId", "runId", "requestKey", "mode"], "additionalProperties": false}}),
        json!({"name": "fail_generation", "description": "Report a failure for the currently leased attempt. Supply a brief diagnostic suitable for the activity log.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "attempt": id, "detail": {"type": "string", "minLength": 1, "maxLength": 2000}
            }, "required": ["workspaceId", "runId", "attempt", "detail"], "additionalProperties": false}}),
        json!({"name": "get_generation_activity", "description": "Read the transition history of a run.",
            "inputSchema": run, "annotations": {"readOnlyHint": true}}),
        json!({"name": "claim_generation", "description": "Claim the next available run in a workspace and receive its attempt number, lease expiry, checkpoint, current step, and any pending Preview or Supplemental Outputs replacements.",
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
        json!({"name": "review_preview_formats",
            "description": "Submit one decision per frozen format for a pending rendered Preview. Only unresolved formats are rerendered.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id, "notes": {"type": "string"},
                "decisions": {"type": "array", "minItems": 1, "items": {"type": "object", "properties": {
                    "format_id": id, "decision": {"enum": ["approved", "revise", "rejected"]}, "note": {"type": "string", "maxLength": 4096}
                }, "required": ["format_id", "decision"], "additionalProperties": false}}
            }, "required": ["workspaceId", "runId", "resultId", "decisions"], "additionalProperties": false}}),
        json!({"name": "review_supplemental_items",
            "description": "Submit one decision per item in the pending supplemental result. Arrays follow submission order. Only unresolved items need replacement.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id, "notes": {"type": "string"},
                "review": {"type": "object", "properties": {
                    "text": {"type": "array", "maxItems": 100, "items": {"enum": ["approved", "revise", "rejected"]}},
                    "media": {"type": "array", "maxItems": 100, "items": {"enum": ["approved", "revise", "rejected"]}},
                    "item_notes": {"type": "object", "properties": {
                        "text": {"type": "array", "maxItems": 100, "items": {"type": ["string", "null"], "maxLength": 4096}},
                        "media": {"type": "array", "maxItems": 100, "items": {"type": ["string", "null"], "maxLength": 4096}}
                    }, "required": ["text", "media"], "additionalProperties": false}
                }, "required": ["text", "media"], "additionalProperties": false}
            }, "required": ["workspaceId", "runId", "resultId", "review"], "additionalProperties": false}}),
        json!({"name": "review_production_result", "description": "Submit an explicit approval, rejection, or revision request for a pending result.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": id, "runId": id, "resultId": id,
                "decision": {"enum": ["approved", "rejected", "revise"]}, "notes": {"type": "string"},
                "saveMotionIndexes": {"type": "array", "items": {"type": "integer", "minimum": 0}, "uniqueItems": true,
                    "description": "On Assets approval, save these zero-based motion item indexes. An empty list saves none."}
            }, "required": ["workspaceId", "runId", "resultId", "decision"], "additionalProperties": false}}),
    ]
}

pub(super) fn call(
    store: &mut WorkspaceStore,
    database: &Path,
    name: &str,
    arguments: serde_json::Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    let arguments = Value::Object(arguments);
    Some(match name {
        "create_video_draft" => parse(arguments).and_then(|args: CreateDraftInput| {
            store
                .create_video_draft(args.workspace_id, args.settings)
                .map(|draft| json!({"draft": draft}))
                .map_err(|_| "Could not create draft. Check title, brief, profile and sources")
        }),
        "update_video_draft" => parse(arguments).and_then(|args: UpdateDraftInput| {
            store
                .update_video_draft(args.workspace_id, args.draft_id, args.settings)
                .map(|draft| json!({"draft": draft}))
                .map_err(|_| "Could not update draft. Check the draft, workspace and settings")
        }),
        "delete_video_draft" => parse(arguments).and_then(|args: DeleteDraftInput| {
            store
                .delete_video_draft(args.workspace_id, args.draft_id, &args.confirmed_title)
                .map(|deleted| json!({"deleted": deleted}))
                .map_err(|_| "Could not delete video draft")
        }),
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
        "process_script_stage" => parse(arguments).and_then(|args: RunInput| {
            store
                .process_script_stage(database, args.workspace_id, args.run_id)
                .map(|result_id| json!({"resultId": result_id}))
                .map_err(|_| "The script could not be written. Check the run activity for the reason")
        }),
        "process_supplemental_writing" => parse(arguments).and_then(|args: RunInput| {
            store
                .process_supplemental_writing(database, args.workspace_id, args.run_id)
                .map(|result_id| json!({"resultId": result_id}))
                .map_err(|_| "The titles and descriptions could not be written. Check the run activity for the reason")
        }),
        "process_preview_edit" => parse(arguments).and_then(|args: RunInput| {
            store
                .process_preview_edit(database, args.workspace_id, args.run_id)
                .map(|result_id| json!({"resultId": result_id}))
                .map_err(|_| "The first cut could not be made. Check the run activity for the reason")
        }),
        "process_asset_plan" => parse(arguments).and_then(|args: RunInput| {
            store
                .process_asset_plan(args.workspace_id, args.run_id)
                .map(|result_id| json!({"resultId": result_id}))
                .map_err(|_| "The asset plan could not be made. Check the run activity for the reason")
        }),
        "process_media_stage" => parse(arguments).and_then(|args: MediaStageInput| {
            store
                .process_media_stage(database, args.workspace_id, args.run_id, args.preview_edit.as_ref())
                .map(|work| json!({"work": work}))
                .map_err(|_| "Media processing failed. Check the edit and run activity")
        }),
        "generate_delivery_subtitles" => parse(arguments).and_then(|args: GenerateSubtitlesInput| {
            store
                .generate_delivery_subtitles(database, args.workspace_id, args.run_id, args.format_id)
                .map(|cues| json!({"cues": cues}))
                .map_err(|_| "Could not generate delivery subtitles. Check the run, output, and timed transcription provider")
        }),
        "import_delivery_subtitles" => parse(arguments).and_then(|args: SubtitleInput| {
            let source = Path::new(&args.source);
            if !source.is_absolute() {
                return Err("Subtitle source must be an absolute local path");
            }
            store
                .import_delivery_subtitles(database, args.workspace_id, args.run_id, args.format_id, source)
                .map(|cues| json!({"cues": cues}))
                .map_err(|_| "Could not import subtitles. Check the rendered format and SRT file")
        }),
        "verify_generation_delivery" => parse(arguments).and_then(|args: VerifyDeliveryInput| {
            store
                .verify_generation_delivery(database, args.workspace_id, args.run_id, args.attempt)
                .map(|run| json!({"run": run_summary(&run)}))
                .map_err(|_| "Could not verify delivery. Check the current lease and required media files")
        }),
        "export_generation_delivery" => parse(arguments).and_then(|args: ExportDeliveryInput| {
            let destination = Path::new(&args.destination);
            if !destination.is_absolute() {
                return Err("Export destination must be an absolute local path");
            }
            store
                .export_generation_delivery(database, args.workspace_id, args.run_id, destination)
                .map(|()| json!({"destination": args.destination}))
                .map_err(|_| "Could not export delivery. Check the verified files and choose a new destination")
        }),
        "rename_completed_video" => parse(arguments).and_then(|args: RenameCompletedInput| {
            store
                .rename_completed_video(args.workspace_id, args.run_id, &args.title)
                .map(|title| json!({"title": title}))
                .map_err(|_| "Could not rename completed video. Check the workspace, run and title")
        }),
        "add_completed_video_feedback" => parse(arguments).and_then(|args: CompletedFeedbackInput| {
            store
                .add_completed_video_feedback(args.workspace_id, args.run_id, args.result_id, &args.markdown)
                .map(|feedback| json!({"feedback": feedback}))
                .map_err(|_| "Could not save feedback. Check the completed run, result and Markdown")
        }),
        "recover_delivery_cleanup" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .recover_delivery_cleanup(database, args.workspace_id)
                .map(|report| json!({"cleanup": report}))
                .map_err(|_| "Could not recover delivery cleanup")
        }),
        "set_output_schedule" => parse(arguments).and_then(|args: ScheduleInput| {
            store
                .set_output_schedule(args.workspace_id, args.run_id, args.format_id, args.date.as_deref())
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save schedule. Check the workspace, verified output and calendar date")
        }),
        "list_scheduled_outputs" => parse(arguments).and_then(|args: CalendarInput| {
            store
                .scheduled_outputs(args.workspace_id, args.profile_id, &args.start, &args.end)
                .map(|outputs| json!({"outputs": outputs}))
                .map_err(|_| "Could not list scheduled outputs. Check the date range")
        }),
        "delete_generation" => parse(arguments).and_then(|args: DeleteRunInput| {
            store
                .delete_generation(database, args.workspace_id, args.run_id, &args.confirmed_title)
                .map(|deleted| json!({"deleted": deleted}))
                .map_err(|_| "Could not finish run deletion. Retry with the same IDs and title")
        }),
        "delete_completed_video" => parse(arguments).and_then(|args: DeleteRunInput| {
            store
                .delete_completed_generation(database, args.workspace_id, args.run_id, &args.confirmed_title)
                .map(|deleted| json!({"deleted": deleted}))
                .map_err(|_| "Could not finish completed video deletion. Retry with the same IDs and title")
        }),
        "get_completed_video" => parse(arguments).and_then(|args: RunInput| {
            store
                .completed_video(args.workspace_id, args.run_id)
                .map(|video| json!({"video": video}))
                .map_err(|_| "Could not read completed video")
        }),
        "list_generation_notifications" => parse(arguments).and_then(|args: NotificationInput| {
            let cursor = i64::try_from(args.after_id).map_err(|_| "Invalid notification cursor")?;
            store
                .generation_notifications(args.workspace_id, cursor)
                .map(|notifications| json!({"notifications": notifications}))
                .map_err(|_| "Could not read generation notifications")
        }),
        "read_completion_notification" => parse(arguments).and_then(|args: RunInput| {
            store
                .read_completion_notification(args.workspace_id, args.run_id)
                .map(|changed| json!({"changed": changed}))
                .map_err(|_| "Could not acknowledge completion notification")
        }),
        "get_generation_plan" => parse(arguments).and_then(|args: RunInput| {
            store
                .generation_plan(args.workspace_id, args.run_id)
                .map(|plan| json!({"plan": plan}))
                .map_err(|_| "Could not read generation plan")
        }),
        "replay_generation" => parse(arguments).and_then(|args: ReplayInput| {
            store
                .replay_generation(args.workspace_id, args.run_id, &args.request_key, args.mode)
                .map(|run| json!({"run": run_summary(&run)}))
                .map_err(|_| "Could not replay run. Check the completed run, workspace and request key")
        }),
        "fail_generation" => parse(arguments).and_then(|args: FailureInput| {
            store
                .fail_generation(args.workspace_id, args.run_id, args.attempt, &args.detail)
                .map(|run| json!({"run": run_summary(&run)}))
                .map_err(|_| "Could not record failure. Check the workspace, current lease and diagnostic")
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
        "review_preview_formats" => parse(arguments).and_then(|args: PreviewReviewInput| {
            store.review_preview_formats(args.workspace_id, args.run_id, args.result_id, &args.decisions, &args.notes)
                .map(|()| json!({"submitted": true}))
                .map_err(|_| "Could not review previews. Check the pending result and provide a decision for each format")
        }),
        "review_supplemental_items" => parse(arguments).and_then(|args: SupplementalReviewInput| {
            store.review_supplemental_items(args.workspace_id, args.run_id, args.result_id, &args.review, &args.notes)
                .map(|()| json!({"submitted": true}))
                .map_err(|_| "Could not review supplemental items. Check the pending result and provide a decision for each item")
        }),
        "review_production_result" => parse(arguments).and_then(|args: ReviewInput| {
            store
                .review_production_result_with_motion(
                    args.workspace_id,
                    args.run_id,
                    args.result_id,
                    args.decision,
                    &args.notes,
                    args.save_motion_indexes.as_deref(),
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
