use rusqlite::{Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::format_deliveries::read_settings;
use crate::{GenerationPlan, ProductionStage, WorkspaceStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approved,
    Rejected,
    Revise,
}

impl ReviewDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Revise => "revise",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProductionProgress {
    pub stage_index: usize,
    pub pending_review_id: Option<i64>,
    pub results: Vec<ProductionResult>,
}

#[derive(Debug, Serialize)]
pub struct ProductionResult {
    pub id: i64,
    pub stage_index: usize,
    pub attempt: i64,
    pub result: Value,
    pub decision: Option<String>,
    pub review_notes: Option<String>,
}

impl WorkspaceStore {
    pub fn submit_production_result(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        stage: ProductionStage,
        result: &Value,
    ) -> rusqlite::Result<i64> {
        self.submit_production_result_with(workspace_id, run_id, attempt, stage, result, || Ok(()))
    }

    pub(crate) fn submit_production_result_with(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        stage: ProductionStage,
        result: &Value,
        publish: impl FnOnce() -> rusqlite::Result<()>,
    ) -> rusqlite::Result<i64> {
        if !result.is_object() || stage == ProductionStage::VerifyDelivery {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let (index, plan): (i64, GenerationPlan) = tx.query_row(
            "SELECT stage_index, plan_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
               AND state = 'running' AND lease_until > unixepoch() AND plan_json IS NOT NULL",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, read_settings(row.get(1)?, 1)?)),
        )?;
        let step = plan
            .steps
            .get(index as usize)
            .filter(|step| step.stage == stage)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        if stage == ProductionStage::Preview && result.get("cuts").is_some() {
            let source_ids: String = tx.query_row(
                "SELECT json_extract(snapshot_json, '$.draft.source_ids')
                 FROM generation_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )?;
            let source_ids: Vec<i64> = read_settings(Some(source_ids), 0)?;
            crate::clip_render::preview_cuts(result, &source_ids)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        }
        let id = tx.query_row(
            "INSERT INTO production_results (run_id, stage_index, attempt, result_json)
             VALUES (?1, ?2, ?3, ?4) RETURNING id",
            params![run_id, index, attempt, result.to_string()],
            |row| row.get(0),
        )?;
        let next_state = if step.review_required {
            "paused"
        } else {
            "queued"
        };
        let advance = i64::from(!step.review_required);
        let pending = step.review_required.then_some(id);
        tx.query_row(
            "UPDATE generation_runs
             SET state = ?3, stage_index = stage_index + ?4, pending_review_id = ?5,
                 checkpoint_json = NULL, lease_until = NULL
             WHERE workspace_id = ?1 AND id = ?2 AND lease_until > unixepoch()
             RETURNING id",
            params![workspace_id, run_id, next_state, advance, pending],
            |_| Ok(()),
        )?;
        publish()?;
        tx.commit()?;
        Ok(id)
    }

    pub fn review_production_result(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        decision: ReviewDecision,
        notes: &str,
    ) -> rusqlite::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        tx.query_row(
            "UPDATE generation_runs
             SET state = 'queued', stage_index = stage_index + ?4, pending_review_id = NULL
             WHERE workspace_id = ?1 AND id = ?2 AND pending_review_id = ?3 AND state = 'paused'
             RETURNING id",
            params![
                workspace_id,
                run_id,
                result_id,
                i64::from(decision == ReviewDecision::Approved)
            ],
            |_| Ok(()),
        )?;
        tx.query_row(
            "UPDATE production_results SET decision = ?3, review_notes = ?4
             WHERE run_id = ?1 AND id = ?2 AND decision IS NULL RETURNING id",
            params![run_id, result_id, decision.as_str(), notes],
            |_| Ok(()),
        )?;
        tx.commit()
    }

    pub fn production_progress(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<ProductionProgress> {
        let tx = self.0.unchecked_transaction()?;
        let (stage_index, pending_review_id) = tx.query_row(
            "SELECT stage_index, CASE WHEN state = 'paused' THEN pending_review_id END
             FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, run_id],
            |row| Ok((row.get::<_, i64>(0)? as usize, row.get(1)?)),
        )?;
        let results = tx
            .prepare(
                "SELECT id, stage_index, attempt, result_json, decision, review_notes
             FROM production_results WHERE run_id = ?1 ORDER BY id",
            )?
            .query_map([run_id], |row| {
                Ok(ProductionResult {
                    id: row.get(0)?,
                    stage_index: row.get::<_, i64>(1)? as usize,
                    attempt: row.get(2)?,
                    result: read_settings(row.get(3)?, 3)?,
                    decision: row.get(4)?,
                    review_notes: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(ProductionProgress {
            stage_index,
            pending_review_id,
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunControl, RunOptions};
    use serde_json::json;

    #[test]
    fn guided_revisions_hold_the_stage_and_autopilot_advances() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
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
                "guided",
                &RunOptions {
                    autopilot: false,
                    platform: None,
                },
            )
            .unwrap();
        let first = store.claim_generation(workspace).unwrap().unwrap();
        store
            .prepare_generation_plan(workspace, run.id, first.attempt)
            .unwrap();
        assert!(
            store
                .submit_production_result(
                    workspace,
                    run.id,
                    first.attempt,
                    ProductionStage::Assets,
                    &json!({})
                )
                .is_err()
        );
        let result = store
            .submit_production_result(
                workspace,
                run.id,
                first.attempt,
                ProductionStage::Script,
                &json!({"markdown":"First"}),
            )
            .unwrap();
        assert!(store.claim_generation(workspace).unwrap().is_none());
        assert!(
            store
                .control_generation(workspace, run.id, RunControl::Resume)
                .is_err()
        );
        assert!(
            store
                .review_production_result(other, run.id, result, ReviewDecision::Approved, "")
                .is_err()
        );
        store
            .review_production_result(
                workspace,
                run.id,
                result,
                ReviewDecision::Revise,
                "Add details",
            )
            .unwrap();
        assert!(
            store
                .review_production_result(workspace, run.id, result, ReviewDecision::Approved, "")
                .is_err()
        );
        let second = store.claim_generation(workspace).unwrap().unwrap();
        assert!(
            store
                .submit_production_result(
                    workspace,
                    run.id,
                    first.attempt,
                    ProductionStage::Script,
                    &json!({})
                )
                .is_err()
        );
        let revised = store
            .submit_production_result(
                workspace,
                run.id,
                second.attempt,
                ProductionStage::Script,
                &json!({"markdown":"Revised"}),
            )
            .unwrap();
        store
            .review_production_result(workspace, run.id, revised, ReviewDecision::Approved, "")
            .unwrap();
        let progress = store.production_progress(workspace, run.id).unwrap();
        assert_eq!(progress.stage_index, 1);
        assert_eq!(progress.pending_review_id, None);
        assert_eq!(
            progress.results[0].review_notes.as_deref(),
            Some("Add details")
        );
        assert_eq!(progress.results[1].decision.as_deref(), Some("approved"));
        store
            .control_generation(workspace, run.id, RunControl::Cancel)
            .unwrap();

        let auto = store
            .enqueue_video_draft(
                workspace,
                draft.id,
                "auto",
                &RunOptions {
                    autopilot: true,
                    platform: None,
                },
            )
            .unwrap();
        let claim = store.claim_generation(workspace).unwrap().unwrap();
        store
            .prepare_generation_plan(workspace, auto.id, claim.attempt)
            .unwrap();
        store
            .submit_production_result(
                workspace,
                auto.id,
                claim.attempt,
                ProductionStage::Script,
                &json!({"markdown":"Autopilot"}),
            )
            .unwrap();
        let progress = store.production_progress(workspace, auto.id).unwrap();
        assert_eq!(progress.stage_index, 1);
        assert_eq!(progress.pending_review_id, None);
    }
}
