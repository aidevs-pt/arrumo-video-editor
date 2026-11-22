//! Plugin actions that run on their own when something happens in a workspace. Each hook is
//! switched by a boolean setting of its plugin, so nothing runs until the creator turns it on.
//! A hook queues an ordinary plugin action job whose ID derives from the event, so an event
//! that is reported twice still runs each hook once.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    HookEvent, MAX_PLUGIN_IO_BYTES, WorkspaceStore, plugin_action_jobs::PluginActionJobRequest,
};

impl WorkspaceStore {
    /// Queues every switched-on `video-completed` hook for a run that has just completed.
    /// Returns how many hook jobs exist for the run afterwards.
    pub fn queue_video_completed_hooks(&self, workspace_id: i64, run_id: i64) -> usize {
        self.queue_run_hooks(workspace_id, run_id, HookEvent::VideoCompleted, "", || {
            self.video_completed_event(workspace_id, run_id)
        })
    }

    /// Queues every switched-on `generation-failed` hook for an attempt that has just failed.
    /// A retried run that fails again is a new event.
    pub fn queue_generation_failed_hooks(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        problem: &str,
    ) -> usize {
        self.queue_run_hooks(
            workspace_id,
            run_id,
            HookEvent::GenerationFailed,
            &attempt.to_string(),
            || {
                let title: Option<String> = self
                    .0
                    .query_row(
                        "SELECT COALESCE(display_title, json_extract(snapshot_json, '$.draft.title'))
                         FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
                        [workspace_id, run_id],
                        |row| row.get(0),
                    )
                    .ok()?;
                let profile: Option<String> = self
                    .0
                    .query_row(
                        "SELECT json_extract(snapshot_json, '$.profile.name') FROM generation_runs
                         WHERE workspace_id = ?1 AND id = ?2",
                        [workspace_id, run_id],
                        |row| row.get(0),
                    )
                    .ok()?;
                let problem: String = problem.chars().take(2000).collect();
                Some(json!({
                    "event": "generation-failed",
                    "video": {"runId": run_id, "title": title, "profile": profile,
                        "attempt": attempt, "problem": problem},
                }))
            },
        )
    }

    /// Queues switched-on `publish-date-reached` hooks for finished outputs whose planned date
    /// is today or in the past week (so a day the app was closed still counts). Each output and
    /// date runs a hook once. Returns how many outputs were due.
    pub fn queue_publish_date_hooks(&self) -> rusqlite::Result<usize> {
        let hooked = self.plugin_actions().is_ok_and(|actions| {
            actions.iter().any(|entry| {
                entry
                    .action
                    .hooks
                    .iter()
                    .any(|hook| hook.event == HookEvent::PublishDateReached)
            })
        });
        if !hooked {
            return Ok(0);
        }
        let due: Vec<(i64, i64, i64, String)> = self
            .0
            .prepare(
                "SELECT run.workspace_id, schedule.run_id, schedule.format_id, schedule.scheduled_date
                 FROM output_schedules schedule JOIN generation_runs run ON run.id = schedule.run_id
                 WHERE run.state = 'completed'
                   AND schedule.scheduled_date <= date('now', 'localtime')
                   AND schedule.scheduled_date >= date('now', 'localtime', '-7 days')
                 ORDER BY schedule.scheduled_date, schedule.run_id",
            )?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (workspace_id, run_id, format_id, date) in &due {
            self.queue_run_hooks(
                *workspace_id,
                *run_id,
                HookEvent::PublishDateReached,
                &format!("{format_id}\0{date}"),
                || {
                    let mut event = self.video_completed_event(*workspace_id, *run_id)?;
                    let output = event["video"]["outputs"]
                        .as_array()?
                        .iter()
                        .find(|output| output["formatId"] == *format_id)?
                        .clone();
                    event["event"] = json!("publish-date-reached");
                    event["video"]["outputs"] = json!([output]);
                    Some(event)
                },
            );
        }
        Ok(due.len())
    }

    fn queue_run_hooks(
        &self,
        workspace_id: i64,
        run_id: i64,
        event: HookEvent,
        occurrence: &str,
        input: impl Fn() -> Option<Value>,
    ) -> usize {
        let Ok(actions) = self.plugin_actions() else {
            return 0;
        };
        let skipped: Vec<String> = self
            .0
            .query_row(
                "SELECT json_extract(snapshot_json, '$.options.skip_hooks') FROM generation_runs
                 WHERE workspace_id = ?1 AND id = ?2",
                [workspace_id, run_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        let mut event_input = None;
        let mut queued = 0;
        for entry in actions {
            if skipped.contains(&format!("{}/{}", entry.plugin_id, entry.action.id)) {
                continue;
            }
            if !entry.action.hooks.iter().any(|hook| {
                hook.event == event
                    && self.hook_switched_on(&entry.plugin_id, workspace_id, &hook.enabled_by)
            }) {
                continue;
            }
            if event_input.is_none() {
                event_input = input();
            }
            let Some(input) = event_input.clone() else {
                return queued;
            };
            let mut key = format!("{}\0{workspace_id}\0{run_id}", event.name());
            if !occurrence.is_empty() {
                key = format!("{key}\0{occurrence}");
            }
            let request = PluginActionJobRequest {
                id: hook_job_id(&key, &entry.plugin_id, &entry.action.id),
                workspace_id: Some(workspace_id),
                plugin_id: entry.plugin_id,
                action_id: entry.action.id,
                package_sha256: entry.package_sha256,
                input,
            };
            if self.queue_plugin_action_job(&request).is_ok() {
                queued += 1;
            }
        }
        queued
    }

    fn hook_switched_on(&self, plugin_id: &str, workspace_id: i64, setting: &str) -> bool {
        self.plugin_settings(plugin_id, Some(workspace_id))
            .ok()
            .and_then(|settings| settings.get(setting)?.effective.clone())
            == Some(Value::Bool(true))
    }

    /// What a `video-completed` hook receives: the finished video's title, texts and outputs.
    /// The texts are left out when they would push the input past the plugin limit.
    fn video_completed_event(&self, workspace_id: i64, run_id: i64) -> Option<Value> {
        let video = self.completed_video(workspace_id, run_id).ok()?;
        let format_name = |id: i64| {
            video.run.snapshot["formats"]
                .as_array()
                .and_then(|formats| formats.iter().find(|entry| entry["format"]["id"] == id))
                .and_then(|entry| entry["format"]["name"].as_str())
                .map(str::to_owned)
        };
        let outputs: Vec<Value> = video
            .delivery
            .verified_outputs
            .iter()
            .map(|output| {
                json!({
                    "formatId": output.format_id,
                    "format": format_name(output.format_id),
                    "durationSeconds": output.duration_seconds,
                    "scheduledFor": video.output_dates.get(&output.format_id),
                })
            })
            .collect();
        let mut event = json!({
            "event": "video-completed",
            "video": {
                "runId": run_id,
                "title": video.title,
                "profile": video.run.snapshot["profile"]["name"],
                "outputs": outputs,
                "texts": video.supplemental_text,
            },
        });
        if envelope_len(&event) > MAX_PLUGIN_IO_BYTES {
            event["video"]["texts"] = json!([]);
        }
        (envelope_len(&event) <= MAX_PLUGIN_IO_BYTES).then_some(event)
    }
}

fn envelope_len(input: &Value) -> usize {
    // The action envelope adds the action ID; leave room for the longest one.
    serde_json::to_vec(input).map_or(usize::MAX, |bytes| bytes.len() + 128)
}

fn hook_job_id(occurrence: &str, plugin_id: &str, action_id: &str) -> String {
    let digest = Sha256::digest(format!("{occurrence}\0{plugin_id}\0{action_id}").as_bytes());
    let mut bytes: [u8; 16] = digest[..16]
        .try_into()
        .expect("a SHA-256 digest has 32 bytes");
    // Mark it as a name-based UUID (version 5 layout, RFC 4122 variant).
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook_plugin() -> Vec<u8> {
        hook_plugin_for("video-completed")
    }

    fn hook_plugin_for(event: &str) -> Vec<u8> {
        use std::io::{Cursor, Write};
        let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        package.start_file("manifest.json", options).unwrap();
        package.write_all(r#"{
            "schemaVersion": 9, "id": "fixture-hooks", "name": "Fixture", "description": "Fixture",
            "capabilities": {"actions": {"name": "Actions", "description": "Fixture"}},
            "settings": [{"id": "auto-upload", "label": "Upload when ready", "description": "Fixture",
                          "kind": "boolean", "scope": "workspace", "required": false}],
            "code": {"backend": {"component": "backend/echo.wasm"}},
            "actions": [{"id": "upload", "label": "Upload", "description": "Fixture", "scope": "workspace",
                         "hooks": [{"event": "EVENT", "enabledBy": "auto-upload"}]}]
        }"#.replace("EVENT", event).as_bytes()).unwrap();
        package.start_file("backend/echo.wasm", options).unwrap();
        package
            .write_all(&wat::parse_str(crate::plugin_runtime::tests::ECHO).unwrap())
            .unwrap();
        package.finish().unwrap().into_inner()
    }

    #[test]
    fn a_completed_video_runs_switched_on_hooks_once() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
                    title: "Launch".into(),
                    intent: "Fixture".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "fixture",
                &crate::RunOptions::default(),
            )
            .unwrap()
            .id;
        let delivery = json!({"verified_outputs": [{"format_id": 5, "video": "5.mp4",
            "subtitles": "5.srt", "duration_seconds": 12.5, "subtitle_cues": 3}]});
        store.0.execute_batch(&format!(
            "UPDATE generation_runs SET state = 'completed', stage_index = 1,
                 plan_json = '{{\"steps\": [{{\"stage\": \"verify_delivery\", \"review_required\": false}}]}}',
                 snapshot_json = json_set(snapshot_json, '$.formats', json('[{{\"format\": {{\"id\": 5, \"name\": \"Landscape\"}}}}]'))
             WHERE id = {run};
             INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES ({run}, 0, 1, '{delivery}');"
        )).unwrap();
        store.install_plugin_zip(hook_plugin().as_slice()).unwrap();

        assert_eq!(
            store.queue_video_completed_hooks(workspace, run),
            0,
            "off until switched on"
        );
        store
            .set_plugin_setting(
                "fixture-hooks",
                "auto-upload",
                Some(workspace),
                Some(&Value::Bool(true)),
            )
            .unwrap();
        assert_eq!(store.queue_video_completed_hooks(workspace, run), 1);
        assert_eq!(
            store.queue_video_completed_hooks(workspace, run),
            1,
            "a repeated event reuses the job"
        );
        let jobs = store.list_plugin_action_jobs(Some(workspace)).unwrap();
        assert_eq!(jobs.len(), 1);

        let job = store.process_next_plugin_action_job().unwrap().unwrap();
        let result = serde_json::to_value(&job).unwrap();
        assert_eq!(result["state"], "completed");
        let input = &result["result"]["input"];
        assert_eq!(input["event"], "video-completed");
        assert_eq!(input["video"]["title"], "Launch");
        assert_eq!(input["video"]["outputs"][0]["format"], "Landscape");
        assert_eq!(input["video"]["outputs"][0]["durationSeconds"], 12.5);

        let skipping = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "skipping",
                &crate::RunOptions {
                    skip_hooks: vec!["fixture-hooks/upload".into()],
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        store
            .0
            .execute_batch(&format!(
                "UPDATE generation_runs SET state = 'completed', stage_index = 1,
                 plan_json = (SELECT plan_json FROM generation_runs WHERE id = {run})
             WHERE id = {skipping};
             INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES ({skipping}, 0, 1, '{delivery}');"
            ))
            .unwrap();
        assert_eq!(
            store.queue_video_completed_hooks(workspace, skipping),
            0,
            "this video skips the hook"
        );
        assert!(
            store
                .enqueue_video_draft(
                    workspace,
                    draft.id,
                    "bad",
                    &crate::RunOptions {
                        skip_hooks: vec!["no slash".into()],
                        ..Default::default()
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn each_failed_attempt_runs_failure_hooks_once() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
                    title: "Launch".into(),
                    intent: "Fixture".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "fixture",
                &crate::RunOptions::default(),
            )
            .unwrap()
            .id;
        store
            .install_plugin_zip(hook_plugin_for("generation-failed").as_slice())
            .unwrap();
        store
            .set_plugin_setting(
                "fixture-hooks",
                "auto-upload",
                Some(workspace),
                Some(&Value::Bool(true)),
            )
            .unwrap();
        assert_eq!(store.queue_video_completed_hooks(workspace, run), 0);
        assert_eq!(
            store.queue_generation_failed_hooks(workspace, run, 1, "Agent stopped"),
            1
        );
        assert_eq!(
            store.queue_generation_failed_hooks(workspace, run, 1, "Agent stopped"),
            1
        );
        assert_eq!(
            store.queue_generation_failed_hooks(workspace, run, 2, "Again"),
            1
        );
        assert_eq!(
            store
                .list_plugin_action_jobs(Some(workspace))
                .unwrap()
                .len(),
            2
        );
        let job = store.process_next_plugin_action_job().unwrap().unwrap();
        let input = &serde_json::to_value(&job).unwrap()["result"]["input"];
        assert_eq!(input["event"], "generation-failed");
        assert_eq!(input["video"]["title"], "Launch");
        assert_eq!(input["video"]["attempt"], 1);
        assert_eq!(input["video"]["problem"], "Agent stopped");
    }

    #[test]
    fn a_planned_publish_date_runs_its_hook_once_per_output() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
                    title: "Launch".into(),
                    intent: "Fixture".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "fixture",
                &crate::RunOptions::default(),
            )
            .unwrap()
            .id;
        let delivery = json!({"verified_outputs": [
            {"format_id": 5, "video": "5.mp4", "subtitles": "5.srt", "duration_seconds": 12.5, "subtitle_cues": 3},
            {"format_id": 6, "video": "6.mp4", "subtitles": "6.srt", "duration_seconds": 9.0, "subtitle_cues": 2}]});
        store.0.execute_batch(&format!(
            "UPDATE generation_runs SET state = 'completed', stage_index = 1,
                 plan_json = '{{\"steps\": [{{\"stage\": \"verify_delivery\", \"review_required\": false}}]}}'
             WHERE id = {run};
             INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES ({run}, 0, 1, '{delivery}');
             INSERT INTO output_schedules (run_id, format_id, scheduled_date) VALUES
                 ({run}, 5, date('now', 'localtime')), ({run}, 6, date('now', 'localtime', '-30 days'));"
        )).unwrap();
        assert_eq!(
            store.queue_publish_date_hooks().unwrap(),
            0,
            "no plugin listens yet"
        );
        store
            .install_plugin_zip(hook_plugin_for("publish-date-reached").as_slice())
            .unwrap();
        store
            .set_plugin_setting(
                "fixture-hooks",
                "auto-upload",
                Some(workspace),
                Some(&Value::Bool(true)),
            )
            .unwrap();
        assert_eq!(
            store.queue_publish_date_hooks().unwrap(),
            1,
            "a month-old date is not due"
        );
        assert_eq!(store.queue_publish_date_hooks().unwrap(), 1);
        assert_eq!(
            store
                .list_plugin_action_jobs(Some(workspace))
                .unwrap()
                .len(),
            1
        );
        let job = store.process_next_plugin_action_job().unwrap().unwrap();
        let input = &serde_json::to_value(&job).unwrap()["result"]["input"];
        assert_eq!(input["event"], "publish-date-reached");
        assert_eq!(input["video"]["outputs"].as_array().unwrap().len(), 1);
        assert_eq!(input["video"]["outputs"][0]["formatId"], 5);
        store.queue_publish_date_hooks().unwrap();
        assert_eq!(
            store
                .list_plugin_action_jobs(Some(workspace))
                .unwrap()
                .len(),
            1,
            "never twice"
        );
    }

    #[test]
    fn hook_jobs_have_one_stable_id_per_event_and_action() {
        let id = hook_job_id("video-completed\x001\x003", "pt.aidevs.example", "upload");
        assert_eq!(
            id,
            hook_job_id("video-completed\x001\x003", "pt.aidevs.example", "upload")
        );
        assert_ne!(
            id,
            hook_job_id("video-completed\x001\x004", "pt.aidevs.example", "upload")
        );
        assert_ne!(
            id,
            hook_job_id("video-completed\x001\x003", "pt.aidevs.example", "publish")
        );
        assert_eq!(id.parse::<Uuid>().unwrap().get_version_num(), 5);
    }
}
