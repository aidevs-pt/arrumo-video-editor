//! Shared driving for production stages the run's agent performs: finding runs that are ready,
//! claiming the stage under a lease, and failing the run with a readable reason when it cannot finish.

use std::{error::Error, io};

use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{
    AgentPlugin, AgentRunError, GenerationRun, WorkspaceStore, agent_plugin,
    generation_worker::LEASE_SECONDS,
};

/// A stage failure whose message is safe and useful to show in the run's activity.
#[derive(Debug)]
pub(crate) struct StageFailure(pub String);

impl std::fmt::Display for StageFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for StageFailure {}

/// Whether the agent can run unattended on this computer.
pub(crate) fn agent_ready(plugin: &AgentPlugin) -> bool {
    plugin.cli.structured_run.is_some() && crate::agent_cli::agent_cli_detected(&plugin.id)
}

impl WorkspaceStore {
    /// Runs waiting at `stage` whose agent is ready, oldest first.
    pub(crate) fn runs_ready_for_agent(&self, stage: &str) -> rusqlite::Result<Vec<(i64, i64)>> {
        let candidates: Vec<(i64, i64, Option<String>)> = self
            .0
            .prepare(
                "SELECT workspace_id, id, json_extract(snapshot_json, '$.agent_settings.effective_provider')
                 FROM generation_runs
                 WHERE (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
                   AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = ?1
                 ORDER BY id",
            )?
            .query_map([stage], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(candidates
            .into_iter()
            .filter(|(_, _, provider)| {
                provider
                    .as_deref()
                    .and_then(agent_plugin)
                    .is_some_and(agent_ready)
            })
            .map(|(workspace_id, run_id, _)| (workspace_id, run_id))
            .collect())
    }

    /// Claims the run's queued `stage`, runs `work` with the attempt, and fails the run with the
    /// agent's reason (or `fallback`) if it does not finish. Returns `None` when the run is not
    /// waiting at that stage.
    pub(crate) fn run_agent_stage(
        &self,
        workspace_id: i64,
        run_id: i64,
        stage: &str,
        fallback: &str,
        work: impl FnOnce(&GenerationRun, &AgentPlugin, i64) -> Result<i64, Box<dyn Error>>,
    ) -> Result<Option<i64>, Box<dyn Error>> {
        let Some(run) = self.generation_run(workspace_id, run_id)? else {
            return Ok(None);
        };
        let provider = run.snapshot["agent_settings"]["effective_provider"]
            .as_str()
            .and_then(agent_plugin)
            .ok_or_else(|| io::Error::other("The run has no agent selected"))?;
        let Some(attempt) = self.claim_queued_stage(workspace_id, run_id, stage)? else {
            return Ok(None);
        };
        if !self.plugin_is_active(&provider.id)? {
            let reason = format!(
                "The run's agent, {}, is turned off. Turn it on in Settings › Plugins, then retry.",
                provider.label
            );
            match self.fail_generation(workspace_id, run_id, attempt, &reason) {
                Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(failure) => return Err(failure.into()),
            }
            return Err(io::Error::other(reason).into());
        }
        // Thumbnails, cuts and clips can outlast the lease between the agent's own renewals; a
        // lapsed lease would let the step be claimed again and start over.
        let keeper = self.0.path().filter(|path| !path.is_empty()).map(|path| {
            crate::lease_keeper::LeaseKeeper::start(
                std::path::Path::new(path),
                workspace_id,
                run_id,
                attempt,
            )
        });
        let timing = crate::telemetry::time_step(stage, &[("agent", provider.id.clone().into())]);
        let outcome = work(&run, provider, attempt);
        timing.finish(outcome.is_ok());
        drop(keeper);
        match outcome {
            Ok(id) => Ok(Some(id)),
            Err(error) => {
                // Steps wrap what failed, so the reason may be anywhere in the chain.
                let reason = crate::quiet_process::error_sources(&*error)
                    .find_map(|error| {
                        error
                            .downcast_ref::<AgentRunError>()
                            .map(ToString::to_string)
                            .or_else(|| {
                                error
                                    .downcast_ref::<StageFailure>()
                                    .map(ToString::to_string)
                            })
                    })
                    .unwrap_or_else(|| fallback.to_owned());
                let cause = crate::error_chain(&*error);
                eprintln!("Arrumo: the {stage} step of run {run_id} failed: {reason}\n{cause}");
                crate::telemetry::warn("A generation step failed", &[("stage", stage.into())]);
                match self.fail_generation_because(
                    workspace_id,
                    run_id,
                    attempt,
                    &reason,
                    (cause != reason).then_some(cause.as_str()),
                ) {
                    Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(failure) => return Err(failure.into()),
                }
                Err(error)
            }
        }
    }

    pub(crate) fn claim_queued_stage(
        &self,
        workspace_id: i64,
        run_id: i64,
        stage: &str,
    ) -> rusqlite::Result<Option<i64>> {
        self.0
            .query_row(
                "UPDATE generation_runs SET state = 'running', attempt = attempt + 1,
                 lease_until = unixepoch() + ?3
                 WHERE workspace_id = ?1 AND id = ?2
                   AND (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
                   AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = ?4
                 RETURNING attempt",
                params![workspace_id, run_id, LEASE_SECONDS, stage],
                |row| row.get(0),
            )
            .optional()
    }

    /// The latest accepted result of an earlier stage, such as the approved script: approved by
    /// the creator, or finished without review on autopilot.
    pub(crate) fn approved_stage_result(
        &self,
        run_id: i64,
        stage: &str,
    ) -> rusqlite::Result<Option<Value>> {
        self.0
            .query_row(
                "SELECT result.result_json FROM production_results AS result
                 JOIN generation_runs AS run ON run.id = result.run_id
                 WHERE result.run_id = ?1
                   AND (result.decision = 'approved'
                        OR (result.decision IS NULL AND result.stage_index < run.stage_index
                            AND json_extract(run.plan_json, '$.steps[' || result.stage_index || '].review_required') = 0))
                   AND json_extract(run.plan_json, '$.steps[' || result.stage_index || '].stage') = ?2
                 ORDER BY result.id DESC LIMIT 1",
                params![run_id, stage],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|text| serde_json::from_str(&text).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error))))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use crate::{DraftSettings, ProductionStage, RunOptions, WorkspaceStore};

    #[test]
    fn autopilot_results_count_as_approved_once_their_step_is_done() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Tips".into(),
                    intent: "Three tips".into(),
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
                    autopilot: true,
                    ..RunOptions::default()
                },
            )
            .unwrap();
        let attempt = store.claim_generation(workspace).unwrap().unwrap().attempt;
        let script = serde_json::json!({"markdown": "## Hook"});
        store
            .submit_production_result(workspace, run.id, attempt, ProductionStage::Script, &script)
            .unwrap();
        assert_eq!(
            store.approved_stage_result(run.id, "script").unwrap(),
            Some(script)
        );
        assert_eq!(store.approved_stage_result(run.id, "assets").unwrap(), None);
    }
}
