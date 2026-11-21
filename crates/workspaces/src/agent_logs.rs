//! A saved record of every agent run: the prompt, the command, what the agent printed and how it
//! ended, so creators can see why a step failed or what the agent was thinking.

use std::{
    error::Error,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

use crate::{
    AgentPlugin, AgentRunError, WorkspaceStore,
    agent_run::{AgentTranscript, run_structured},
};

/// How often a running log is updated with the agent's latest output.
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);

/// Where an agent run belongs: a workspace task, or a stage attempt of a generation run.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AgentLogScope<'a> {
    pub workspace_id: i64,
    pub run_id: Option<i64>,
    pub stage: &'a str,
    pub attempt: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogSummary {
    pub id: i64,
    pub run_id: Option<i64>,
    pub stage: String,
    pub attempt: Option<i64>,
    pub provider: String,
    pub status: String,
    pub started_at: i64,
    pub duration_ms: Option<i64>,
    pub problem: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLog {
    #[serde(flatten)]
    pub summary: AgentLogSummary,
    pub exit_code: Option<i64>,
    pub command: String,
    pub prompt: String,
    pub output: String,
    pub errors: String,
    pub answer: Option<String>,
}

fn status(result: &Result<Value, AgentRunError>) -> &'static str {
    match result {
        Ok(_) => "succeeded",
        Err(AgentRunError::TimedOut) => "timed_out",
        Err(AgentRunError::Unavailable | AgentRunError::Unsupported) => "unavailable",
        Err(AgentRunError::Failed(_)) => "failed",
    }
}

const SUMMARY_COLUMNS: &str =
    "id, run_id, stage, attempt, provider, status, started_at, duration_ms, problem";

fn read_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentLogSummary> {
    Ok(AgentLogSummary {
        id: row.get(0)?,
        run_id: row.get(1)?,
        stage: row.get(2)?,
        attempt: row.get(3)?,
        provider: row.get(4)?,
        status: row.get(5)?,
        started_at: row.get(6)?,
        duration_ms: row.get(7)?,
        problem: row.get(8)?,
    })
}

impl WorkspaceStore {
    /// Runs the agent like `run_structured` and records the run in the workspace's agent logs.
    /// Logging problems never fail the run itself.
    pub(crate) fn run_logged_agent(
        &self,
        scope: AgentLogScope<'_>,
        provider: &AgentPlugin,
        prompt: &str,
        schema: &Value,
        timeout: Duration,
        mut tick: impl FnMut() -> Result<(), Box<dyn Error>>,
    ) -> Result<Value, AgentRunError> {
        // An earlier attempt that never finished was interrupted by a crash or restart.
        let _ = self.0.execute(
            "UPDATE agent_logs SET status = 'failed', problem = 'Interrupted before it finished'
             WHERE workspace_id = ?1 AND run_id IS ?2 AND stage = ?3 AND status = 'running'",
            params![scope.workspace_id, scope.run_id, scope.stage],
        );
        let log: Option<i64> = self
            .0
            .query_row(
                "INSERT INTO agent_logs (workspace_id, run_id, stage, attempt, provider, prompt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id",
                params![
                    scope.workspace_id,
                    scope.run_id,
                    scope.stage,
                    scope.attempt,
                    provider.id,
                    prompt
                ],
                |row| row.get(0),
            )
            .ok();
        let transcript = AgentTranscript::default();
        let started = Instant::now();
        let mut flushed = Instant::now();
        let result = run_structured(provider, prompt, schema, timeout, &transcript, || {
            if let Some(id) = log
                && flushed.elapsed() >= FLUSH_INTERVAL
            {
                flushed = Instant::now();
                let _ = self.0.execute(
                    "UPDATE agent_logs SET command = ?2, output = ?3, errors = ?4,
                     duration_ms = ?5 WHERE id = ?1",
                    params![
                        id,
                        transcript
                            .command
                            .lock()
                            .map(|c| c.clone())
                            .unwrap_or_default(),
                        transcript.output_text(),
                        transcript.errors_text(),
                        started.elapsed().as_millis() as i64
                    ],
                );
            }
            tick()
        });
        if let Some(id) = log {
            let answer = result
                .as_ref()
                .ok()
                .and_then(|answer| serde_json::to_string_pretty(answer).ok());
            let problem = result.as_ref().err().map(ToString::to_string);
            let _ = self.0.execute(
                "UPDATE agent_logs SET status = ?2, command = ?3, output = ?4, errors = ?5,
                 duration_ms = ?6, exit_code = ?7, answer = ?8, problem = ?9 WHERE id = ?1",
                params![
                    id,
                    status(&result),
                    transcript
                        .command
                        .lock()
                        .map(|c| c.clone())
                        .unwrap_or_default(),
                    transcript.output_text(),
                    transcript.errors_text(),
                    started.elapsed().as_millis() as i64,
                    transcript.exit_code.lock().ok().and_then(|code| *code),
                    answer,
                    problem
                ],
            );
        }
        result
    }

    /// The workspace's agent runs, newest first, optionally only those of one generation run.
    pub fn agent_logs(
        &self,
        workspace_id: i64,
        run_id: Option<i64>,
        limit: u32,
    ) -> rusqlite::Result<Vec<AgentLogSummary>> {
        self.0
            .prepare(&format!(
                "SELECT {SUMMARY_COLUMNS} FROM agent_logs
                 WHERE workspace_id = ?1 AND (?2 IS NULL OR run_id = ?2)
                 ORDER BY id DESC LIMIT ?3"
            ))?
            .query_map(
                params![workspace_id, run_id, limit.clamp(1, 500)],
                read_summary,
            )?
            .collect()
    }

    /// One agent run with its prompt and everything it printed.
    pub fn agent_log(&self, workspace_id: i64, id: i64) -> rusqlite::Result<Option<AgentLog>> {
        self.0
            .query_row(
                &format!(
                    "SELECT {SUMMARY_COLUMNS}, exit_code, command, prompt, output, errors, answer
                     FROM agent_logs WHERE workspace_id = ?1 AND id = ?2"
                ),
                params![workspace_id, id],
                |row| {
                    Ok(AgentLog {
                        summary: read_summary(row)?,
                        exit_code: row.get(9)?,
                        command: row.get(10)?,
                        prompt: row.get(11)?,
                        output: row.get(12)?,
                        errors: row.get(13)?,
                        answer: row.get(14)?,
                    })
                },
            )
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_are_scoped_to_their_workspace_and_run() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let id: i64 = store
            .0
            .query_row(
                "INSERT INTO agent_logs (workspace_id, stage, provider, prompt, status, errors)
                 VALUES (?1, 'context', 'codex', 'Prompt text', 'failed', 'boom') RETURNING id",
                [workspace],
                |row| row.get(0),
            )
            .unwrap();
        let logs = store.agent_logs(workspace, None, 50).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].status, "failed");
        assert!(store.agent_logs(other, None, 50).unwrap().is_empty());
        assert!(store.agent_logs(workspace, Some(1), 50).unwrap().is_empty());
        let log = store.agent_log(workspace, id).unwrap().unwrap();
        assert_eq!(
            (log.prompt.as_str(), log.errors.as_str()),
            ("Prompt text", "boom")
        );
        assert!(store.agent_log(other, id).unwrap().is_none());
    }

    #[test]
    fn statuses_follow_how_the_run_ended() {
        assert_eq!(status(&Ok(Value::Null)), "succeeded");
        assert_eq!(status(&Err(AgentRunError::TimedOut)), "timed_out");
        assert_eq!(status(&Err(AgentRunError::Unavailable)), "unavailable");
        assert_eq!(status(&Err(AgentRunError::Failed("x"))), "failed");
    }
}
