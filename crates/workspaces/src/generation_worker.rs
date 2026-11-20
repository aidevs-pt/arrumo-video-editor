use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::format_deliveries::read_settings;
use crate::generation_queue::read_run;
use crate::{
    GenerationPlan, GenerationRun, PlanStep, ProductionStage, SupplementalRevisionRequest,
    WorkspaceStore,
};

pub(crate) const LEASE_SECONDS: i64 = 60;

#[derive(Debug, Serialize)]
pub struct GenerationClaim {
    pub run: GenerationRun,
    pub attempt: i64,
    pub lease_until: i64,
    pub checkpoint: Option<Value>,
    pub current_step: Option<PlanStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplemental_revision: Option<SupplementalRevisionRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_revision: Option<crate::PreviewRevisionRequest>,
}

#[derive(Debug, Serialize)]
pub struct GenerationActivity {
    pub attempt: i64,
    pub state: String,
    pub kind: String,
    pub created_at: i64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunControl {
    Pause,
    Resume,
    Retry,
    Cancel,
    /// Returns to the previous reviewed step so its result can be changed.
    Stop,
}

impl WorkspaceStore {
    pub fn claim_generation(&self, workspace_id: i64) -> rusqlite::Result<Option<GenerationClaim>> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let claimed = tx
            .query_row(
                "UPDATE generation_runs
             SET state = 'running', attempt = attempt + 1, lease_until = unixepoch() + ?2
             WHERE workspace_id = ?1 AND id = (
                 SELECT id FROM generation_runs WHERE workspace_id = ?1
                 AND (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
                 ORDER BY id LIMIT 1
             )
             RETURNING id, draft_id, state, snapshot_json, pending_review_id, attempt, lease_until, checkpoint_json, display_title, plan_json, stage_index",
                params![workspace_id, LEASE_SECONDS],
                |row| {
                    let plan: Option<GenerationPlan> = read_settings(row.get("plan_json")?, 9)?;
                    let index = row.get::<_, u32>("stage_index")?;
                    Ok((GenerationClaim {
                        run: read_run(row)?,
                        attempt: row.get(5)?,
                        lease_until: row.get(6)?,
                        checkpoint: read_settings(row.get(7)?, 7)?,
                        current_step: plan.and_then(|plan| plan.steps.into_iter().nth(index as usize)),
                        supplemental_revision: None,
                        preview_revision: None,
                    }, i64::from(index)))
                },
            )
            .optional()?;
        let mut result = None;
        if let Some((mut claim, stage_index)) = claimed {
            match claim.current_step.as_ref().map(|step| step.stage) {
                Some(ProductionStage::SupplementalOutputs) => {
                    claim.supplemental_revision =
                        crate::supplemental_review::supplemental_revision_request(
                            &tx,
                            claim.run.id,
                            stage_index,
                        )?;
                }
                Some(ProductionStage::Preview) => {
                    claim.preview_revision = crate::preview_review::preview_revision_request(
                        &tx,
                        claim.run.id,
                        stage_index,
                    )?;
                }
                _ => {}
            }
            result = Some(claim);
        }
        tx.commit()?;
        Ok(result)
    }

    pub fn renew_generation_lease(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
    ) -> rusqlite::Result<i64> {
        self.0.query_row(
            "UPDATE generation_runs SET lease_until = unixepoch() + ?4
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
             AND state = 'running' AND lease_until > unixepoch() RETURNING lease_until",
            params![workspace_id, run_id, attempt, LEASE_SECONDS],
            |row| row.get(0),
        )
    }

    pub fn checkpoint_generation(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        checkpoint: &Value,
    ) -> rusqlite::Result<i64> {
        self.0.query_row(
            "UPDATE generation_runs
             SET checkpoint_json = ?4, lease_until = unixepoch() + ?5
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
               AND state = 'running' AND lease_until > unixepoch()
             RETURNING lease_until",
            params![
                workspace_id,
                run_id,
                attempt,
                checkpoint.to_string(),
                LEASE_SECONDS
            ],
            |row| row.get(0),
        )
    }

    pub fn fail_generation(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        detail: &str,
    ) -> rusqlite::Result<GenerationRun> {
        let detail = detail.trim();
        if detail.is_empty() || detail.len() > 2000 || detail.contains('\0') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let run = tx.query_row(
            "UPDATE generation_runs SET state = 'failed', lease_until = NULL
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
               AND state = 'running' AND lease_until > unixepoch()
             RETURNING id, draft_id, state, snapshot_json, pending_review_id, display_title",
            params![workspace_id, run_id, attempt],
            read_run,
        )?;
        tx.execute(
            "UPDATE generation_activity SET detail = ?2
             WHERE id = (SELECT max(id) FROM generation_activity WHERE run_id = ?1)",
            params![run_id, detail],
        )?;
        tx.commit()?;
        Ok(run)
    }

    pub fn control_generation(
        &self,
        workspace_id: i64,
        run_id: i64,
        action: RunControl,
    ) -> rusqlite::Result<GenerationRun> {
        let run = self.apply_generation_control(workspace_id, run_id, action)?;
        Ok(self.generation_run(workspace_id, run.id)?.unwrap_or(run))
    }

    fn apply_generation_control(
        &self,
        workspace_id: i64,
        run_id: i64,
        action: RunControl,
    ) -> rusqlite::Result<GenerationRun> {
        let (target, allowed) = match action {
            RunControl::Stop => return self.stop_generation(workspace_id, run_id),
            RunControl::Pause => ("paused", ["queued", "running", "paused"]),
            RunControl::Retry => ("queued", ["failed", "queued", "queued"]),
            RunControl::Resume => ("queued", ["paused", "queued", "queued"]),
            RunControl::Cancel => ("cancelled", ["queued", "running", "paused"]),
        };
        self.0.query_row(
            "UPDATE generation_runs SET state = ?3, lease_until = NULL
             WHERE workspace_id = ?1 AND id = ?2 AND state IN (?4, ?5, ?6)
               AND (?3 != 'queued' OR pending_review_id IS NULL)
             RETURNING id, draft_id, state, snapshot_json, pending_review_id, display_title",
            params![
                workspace_id,
                run_id,
                target,
                allowed[0],
                allowed[1],
                allowed[2]
            ],
            read_run,
        )
    }

    /// Reopens the most recent result of an earlier stage for review and pauses the run there.
    /// The original decision stays in the history; the review applies to a copy of the result.
    fn stop_generation(&self, workspace_id: i64, run_id: i64) -> rusqlite::Result<GenerationRun> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let (previous, stage_index, was_paused): (i64, i64, bool) = tx.query_row(
            "SELECT result.id, result.stage_index, run.state = 'paused' FROM production_results AS result
             JOIN generation_runs AS run ON run.id = result.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2
               AND run.state IN ('queued', 'running', 'paused', 'failed')
               AND result.stage_index < run.stage_index
             ORDER BY result.id DESC LIMIT 1",
            params![workspace_id, run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let reopened: i64 = tx.query_row(
            "INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             SELECT run_id, stage_index, attempt, result_json FROM production_results WHERE id = ?1
             RETURNING id",
            [previous],
            |row| row.get(0),
        )?;
        let run = tx.query_row(
            "UPDATE generation_runs
             SET state = 'paused', stage_index = ?3, pending_review_id = ?4, lease_until = NULL
             WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, draft_id, state, snapshot_json, pending_review_id, display_title",
            params![workspace_id, run_id, stage_index, reopened],
            read_run,
        )?;
        let detail = "Stopped and returned to the previous step";
        if was_paused {
            // A paused run stays paused, so the state trigger records nothing.
            tx.execute(
                "INSERT INTO generation_activity (run_id, attempt, state, kind, detail)
                 SELECT id, attempt, state, 'transition', ?2 FROM generation_runs WHERE id = ?1",
                params![run_id, detail],
            )?;
        } else {
            tx.execute(
                "UPDATE generation_activity SET detail = ?2
                 WHERE id = (SELECT max(id) FROM generation_activity WHERE run_id = ?1)",
                params![run_id, detail],
            )?;
        }
        tx.commit()?;
        Ok(run)
    }

    pub fn generation_activity(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<GenerationActivity>> {
        self.0
            .prepare(
                "SELECT event.attempt, event.state, event.kind, event.created_at, event.detail
             FROM generation_activity AS event JOIN generation_runs AS run ON run.id = event.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 ORDER BY event.id",
            )?
            .query_map(params![workspace_id, run_id], |row| {
                Ok(GenerationActivity {
                    attempt: row.get(0)?,
                    state: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                    detail: row.get(4)?,
                })
            })?
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunOptions};
    use serde_json::json;

    #[test]
    fn expired_and_paused_attempts_cannot_overwrite_checkpoints() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "First".into(),
                    intent: "Demo".into(),
                    profile_id: None,
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "key",
                &RunOptions {
                    autopilot: false,
                    ..RunOptions::default()
                },
            )
            .unwrap();
        assert!(store.claim_generation(other).unwrap().is_none());
        let first = store.claim_generation(workspace).unwrap().unwrap();
        assert_eq!(first.attempt, 1);
        assert!(store.claim_generation(workspace).unwrap().is_none());
        store
            .checkpoint_generation(workspace, run.id, first.attempt, &json!({"step":1}))
            .unwrap();
        store
            .0
            .execute(
                "UPDATE generation_runs SET lease_until = 0 WHERE id = ?1",
                [run.id],
            )
            .unwrap();
        let second = store.claim_generation(workspace).unwrap().unwrap();
        assert_eq!(second.attempt, 2);
        assert_eq!(second.checkpoint, Some(json!({"step":1})));
        assert!(
            store
                .checkpoint_generation(workspace, run.id, first.attempt, &json!({}))
                .is_err()
        );
        store
            .control_generation(workspace, run.id, RunControl::Pause)
            .unwrap();
        assert!(
            store
                .checkpoint_generation(workspace, run.id, second.attempt, &json!({}))
                .is_err()
        );
        assert!(store.claim_generation(workspace).unwrap().is_none());
        store
            .control_generation(workspace, run.id, RunControl::Resume)
            .unwrap();
        assert_eq!(
            store.claim_generation(workspace).unwrap().unwrap().attempt,
            3
        );
        store
            .control_generation(workspace, run.id, RunControl::Cancel)
            .unwrap();
        assert!(store.claim_generation(workspace).unwrap().is_none());
        let states: Vec<_> = store
            .generation_activity(workspace, run.id)
            .unwrap()
            .into_iter()
            .map(|event| event.state)
            .collect();
        assert_eq!(
            states,
            [
                "queued",
                "running",
                "running",
                "paused",
                "queued",
                "running",
                "cancelled"
            ]
        );
        assert!(store.generation_activity(other, run.id).unwrap().is_empty());
    }

    #[test]
    fn failure_is_fenced_and_retry_retains_checkpoint_and_history() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Demo".into(),
                    intent: "Brief".into(),
                    profile_id: None,
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "request",
                &RunOptions {
                    autopilot: false,
                    ..RunOptions::default()
                },
            )
            .unwrap();
        let claim = store.claim_generation(workspace).unwrap().unwrap();
        assert!(
            store
                .fail_generation(workspace, run.id, claim.attempt + 1, "Stale attempt")
                .is_err()
        );
        assert!(
            store
                .fail_generation(workspace, run.id, claim.attempt, " ")
                .is_err()
        );
        store
            .checkpoint_generation(workspace, run.id, claim.attempt, &json!({"step":2}))
            .unwrap();
        let failed = store
            .fail_generation(workspace, run.id, claim.attempt, "Source unavailable")
            .unwrap();
        assert_eq!(failed.state, "failed");
        assert!(store.claim_generation(workspace).unwrap().is_none());
        store
            .control_generation(workspace, run.id, RunControl::Retry)
            .unwrap();
        let retry = store.claim_generation(workspace).unwrap().unwrap();
        assert_eq!(retry.attempt, claim.attempt + 1);
        assert_eq!(retry.checkpoint, Some(json!({"step":2})));
        assert!(
            store
                .fail_generation(workspace, run.id, claim.attempt, "Stale attempt")
                .is_err()
        );
        let history = store.generation_activity(workspace, run.id).unwrap();
        assert_eq!(history[2].state, "failed");
        assert_eq!(history[2].detail.as_deref(), Some("Source unavailable"));
    }
}

#[cfg(test)]
mod stop_tests {
    use crate::{
        DraftSettings, ProductionStage, ReviewDecision, RunControl, RunOptions, WorkspaceStore,
    };

    #[test]
    fn stop_reopens_the_previous_step_and_keeps_its_history() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Explainer".into(),
                    profile_id: None,
                    intent: "Explain".into(),
                    source_ids: vec![],
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "key", &RunOptions::default())
            .unwrap();
        assert!(
            store
                .control_generation(workspace, run.id, RunControl::Stop)
                .is_err(),
            "nothing to go back to yet"
        );

        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let script = serde_json::json!({"markdown": "## Hook"});
        let first = store
            .submit_production_result(workspace, run.id, attempt, ProductionStage::Script, &script)
            .unwrap();
        store
            .review_production_result(workspace, run.id, first, ReviewDecision::Approved, "Good")
            .unwrap();

        let other = store.create("Other").unwrap().id;
        assert!(
            store
                .control_generation(other, run.id, RunControl::Stop)
                .is_err()
        );

        let stopped = store
            .control_generation(workspace, run.id, RunControl::Stop)
            .unwrap();
        assert_eq!(stopped.state, "paused");
        let progress = store.production_progress(workspace, run.id).unwrap();
        assert_eq!(progress.stage_index, 0);
        let reopened = progress.pending_review_id.unwrap();
        assert_ne!(reopened, first);
        assert_eq!(progress.results[0].decision.as_deref(), Some("approved"));
        assert_eq!(progress.results[1].result, script);
        assert!(progress.results[1].decision.is_none());
        let activity = store.generation_activity(workspace, run.id).unwrap();
        assert_eq!(
            activity.last().unwrap().detail.as_deref(),
            Some("Stopped and returned to the previous step")
        );

        store
            .review_production_result(
                workspace,
                run.id,
                reopened,
                ReviewDecision::Revise,
                "Shorter",
            )
            .unwrap();
        assert_eq!(
            store
                .production_progress(workspace, run.id)
                .unwrap()
                .stage_index,
            0
        );
    }
}
