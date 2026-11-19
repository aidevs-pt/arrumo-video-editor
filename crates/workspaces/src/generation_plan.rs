use rusqlite::{Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::format_deliveries::read_settings;
use crate::{RunOptions, SupplementalOutputs, WorkspaceStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionStage {
    Script,
    Assets,
    Preview,
    SupplementalOutputs,
    FinalRender,
    VerifyDelivery,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlanStep {
    pub stage: ProductionStage,
    pub review_required: bool,
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct GenerationPlan {
    pub steps: Vec<PlanStep>,
}

#[derive(Deserialize)]
struct PlanningInput {
    schema_version: u32,
    options: RunOptions,
    formats: Vec<FormatInput>,
}

#[derive(Deserialize)]
struct FormatInput {
    delivery: DeliveryInput,
}

#[derive(Deserialize)]
struct DeliveryInput {
    supplemental: SupplementalOutputs,
}

impl PlanningInput {
    fn into_plan(self) -> rusqlite::Result<GenerationPlan> {
        if self.schema_version != 1 || self.formats.is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let mut stages = vec![
            ProductionStage::Script,
            ProductionStage::Assets,
            ProductionStage::Preview,
        ];
        if self.formats.iter().any(|format| {
            let outputs = &format.delivery.supplemental;
            outputs.thumbnails > 0
                || outputs.titles > 0
                || outputs.descriptions > 0
                || outputs.tags
                || outputs.portrait_derivatives.count > 0
        }) {
            stages.push(ProductionStage::SupplementalOutputs);
        }
        stages.extend([
            ProductionStage::FinalRender,
            ProductionStage::VerifyDelivery,
        ]);
        Ok(GenerationPlan {
            steps: stages
                .into_iter()
                .map(|stage| PlanStep {
                    stage,
                    review_required: !self.options.autopilot
                        && !matches!(
                            stage,
                            ProductionStage::FinalRender | ProductionStage::VerifyDelivery
                        ),
                })
                .collect(),
        })
    }
}

impl WorkspaceStore {
    pub fn prepare_generation_plan(
        &self,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
    ) -> rusqlite::Result<GenerationPlan> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let (snapshot, existing): (String, Option<String>) = tx.query_row(
            "SELECT snapshot_json, plan_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
               AND state = 'running' AND lease_until > unixepoch()",
            params![workspace_id, run_id, attempt],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let plan = if let Some(existing) = existing {
            read_settings(Some(existing), 1)?
        } else {
            let plan = serde_json::from_str::<PlanningInput>(&snapshot)
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?
                .into_plan()?;
            let json = serde_json::to_string(&plan)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            tx.query_row(
                "UPDATE generation_runs SET plan_json = ?4
                 WHERE workspace_id = ?1 AND id = ?2 AND attempt = ?3
                   AND state = 'running' AND lease_until > unixepoch() RETURNING id",
                params![workspace_id, run_id, attempt, json],
                |_| Ok(()),
            )?;
            plan
        };
        tx.commit()?;
        Ok(plan)
    }

    pub fn generation_plan(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Option<GenerationPlan>> {
        self.0.query_row(
            "SELECT plan_json FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, run_id],
            |row| read_settings(row.get(0)?, 0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunControl};

    #[test]
    fn plans_guided_and_autopilot_runs_from_their_frozen_inputs() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Review".into(),
                    intent: "Brief".into(),
                    source_ids: vec![],
                    profile_id: None,
                },
            )
            .unwrap();
        let guided_run = store
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
        let format = store.list_output_formats(workspace).unwrap()[0].id;
        store
            .set_format_delivery(
                workspace,
                format,
                &SupplementalOutputs {
                    thumbnails: 3,
                    ..Default::default()
                },
            )
            .unwrap();
        let auto_run = store
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

        assert!(
            store
                .generation_plan(workspace, guided_run.id)
                .unwrap()
                .is_none()
        );
        let guided = store.claim_generation(workspace).unwrap().unwrap();
        assert!(
            store
                .prepare_generation_plan(workspace, guided_run.id, guided.attempt + 1)
                .is_err()
        );
        let plan = store
            .prepare_generation_plan(workspace, guided_run.id, guided.attempt)
            .unwrap();
        assert_eq!(
            plan.steps.iter().map(|step| step.stage).collect::<Vec<_>>(),
            [
                ProductionStage::Script,
                ProductionStage::Assets,
                ProductionStage::Preview,
                ProductionStage::FinalRender,
                ProductionStage::VerifyDelivery,
            ]
        );
        assert_eq!(
            plan.steps
                .iter()
                .filter(|step| step.review_required)
                .count(),
            3
        );
        assert_eq!(
            store
                .generation_plan(workspace, guided_run.id)
                .unwrap()
                .as_ref(),
            Some(&plan)
        );
        store
            .control_generation(workspace, guided_run.id, RunControl::Pause)
            .unwrap();
        assert!(
            store
                .prepare_generation_plan(workspace, guided_run.id, guided.attempt)
                .is_err()
        );
        let auto = store.claim_generation(workspace).unwrap().unwrap();
        let plan = store
            .prepare_generation_plan(workspace, auto_run.id, auto.attempt)
            .unwrap();
        assert!(
            plan.steps
                .iter()
                .any(|step| step.stage == ProductionStage::SupplementalOutputs)
        );
        assert!(plan.steps.iter().all(|step| !step.review_required));
    }
}
