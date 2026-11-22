//! A saved record of every agent run: the prompt, the command, what the agent printed and how it
//! ended, so creators can see why a step failed or what the agent was thinking.

use std::{
    collections::BTreeMap,
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

/// Something the agent did while it worked, read from its output with its plugin's patterns.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AgentStep {
    pub kind: String,
    pub text: String,
}

/// The steps in an agent's printed output, in order, without immediate repeats.
pub(crate) fn agent_steps(provider: &str, text: &str) -> Vec<AgentStep> {
    let Some(run) =
        crate::agent_plugin(provider).and_then(|plugin| plugin.cli.structured_run.as_ref())
    else {
        return Vec::new();
    };
    let patterns: Vec<(regex::Regex, &crate::plugin_catalog::ProgressPattern)> = run
        .progress
        .iter()
        .filter(|entry| matches!(entry.kind.as_str(), "thinking" | "search" | "step"))
        .filter_map(|entry| {
            regex::RegexBuilder::new(&entry.pattern)
                .size_limit(1 << 20)
                .build()
                .ok()
                .map(|pattern| (pattern, entry))
        })
        .collect();
    let mut steps: Vec<AgentStep> = Vec::new();
    for line in text.lines().map(str::trim_end) {
        let event: Option<Value> = line
            .starts_with('{')
            .then(|| serde_json::from_str(line).ok())
            .flatten();
        for (pattern, entry) in &patterns {
            let subject = match &entry.pointer {
                None => line,
                Some(pointer) => match event.as_ref().and_then(|event| event.pointer(pointer)) {
                    Some(Value::String(text)) => text
                        .lines()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or(""),
                    _ => continue,
                },
            };
            let Some(found) = pattern.captures(subject) else {
                continue;
            };
            let text = found
                .get(1)
                .map(|group| group.as_str().trim().to_owned())
                .or_else(|| entry.label.clone())
                .unwrap_or_default();
            let text: String = text.chars().take(200).collect();
            if !text.is_empty() && steps.last().is_none_or(|last| last.text != text) {
                steps.push(AgentStep {
                    kind: entry.kind.clone(),
                    text,
                });
            }
            break;
        }
        if steps.len() >= 200 {
            break;
        }
    }
    steps
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
    pub steps: Vec<AgentStep>,
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
    /// The agent plugin's non-secret settings for a workspace: chosen values, else the plugin's
    /// defaults. These decide capabilities such as web search, model and reasoning effort.
    pub(crate) fn agent_options(
        &self,
        workspace_id: i64,
        plugin_id: &str,
    ) -> BTreeMap<String, Value> {
        let mut options: BTreeMap<String, Value> = self
            .plugin_manifest(plugin_id)
            .map(|manifest| {
                manifest
                    .settings
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|setting| Some((setting.id, setting.default?)))
                    .collect()
            })
            .unwrap_or_default();
        if let Ok(settings) = self.plugin_settings(plugin_id, Some(workspace_id)) {
            for (id, setting) in settings {
                if let Some(value) = setting.effective {
                    options.insert(id, value);
                }
            }
        }
        options
    }

    /// The options a run froze when it was queued, or the workspace's current ones.
    fn run_agent_options(
        &self,
        scope: AgentLogScope<'_>,
        plugin_id: &str,
    ) -> BTreeMap<String, Value> {
        let frozen = scope.run_id.and_then(|run_id| {
            let text: String = self
                .0
                .query_row(
                    "SELECT json_extract(snapshot_json, '$.agent_options') FROM generation_runs
                     WHERE workspace_id = ?1 AND id = ?2
                       AND json_extract(snapshot_json, '$.agent_settings.effective_provider') = ?3",
                    params![scope.workspace_id, run_id, plugin_id],
                    |row| row.get(0),
                )
                .ok()?;
            serde_json::from_str(&text).ok()
        });
        frozen.unwrap_or_else(|| self.agent_options(scope.workspace_id, plugin_id))
    }

    /// Runs the agent like `run_structured` and records the run in the workspace's agent logs.
    /// Logging problems never fail the run itself.
    pub(crate) fn run_logged_agent(
        &self,
        scope: AgentLogScope<'_>,
        provider: &AgentPlugin,
        prompt: &str,
        schema: &Value,
        timeout: Duration,
        tick: impl FnMut() -> Result<(), Box<dyn Error>>,
    ) -> Result<Value, AgentRunError> {
        self.run_logged_agent_with_images(scope, provider, prompt, schema, timeout, &[], tick)
    }

    /// Runs the agent like `run_logged_agent`, also showing it `images` when its plugin can take
    /// images.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_logged_agent_with_images(
        &self,
        scope: AgentLogScope<'_>,
        provider: &AgentPlugin,
        prompt: &str,
        schema: &Value,
        timeout: Duration,
        images: &[std::path::PathBuf],
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
        let options = self.run_agent_options(scope, &provider.id);
        let transcript = AgentTranscript::default();
        let started = Instant::now();
        let mut flushed = Instant::now();
        let result = run_structured(
            provider,
            prompt,
            schema,
            timeout,
            &options,
            images,
            &transcript,
            || {
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
            },
        );
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
                        steps: Vec::new(),
                    })
                },
            )
            .optional()
            .map(|log| {
                log.map(|mut log| {
                    let printed = format!("{}\n{}", log.errors, log.output);
                    log.steps = agent_steps(&log.summary.provider, &printed);
                    log
                })
            })
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

    #[test]
    fn bundled_agents_declare_their_capability_settings() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        for id in ["codex", "claude-code"] {
            let plugin = crate::agent_plugin(id).unwrap();
            let run = plugin.cli.structured_run.as_ref().unwrap();
            let options = store.agent_options(workspace, id);
            assert_eq!(options.get("web-search"), Some(&Value::Bool(false)), "{id}");
            assert_eq!(
                options.get("reasoning-effort"),
                Some(&Value::String("medium".into()))
            );
            let args = crate::agent_run::setting_arguments(run, &options);
            assert!(!args.is_empty(), "{id} maps its settings to arguments");
        }
        store
            .set_plugin_setting(
                "codex",
                "web-search",
                Some(workspace),
                Some(&Value::Bool(true)),
            )
            .unwrap();
        assert!(
            store
                .set_plugin_setting(
                    "codex",
                    "reasoning-effort",
                    Some(workspace),
                    Some(&Value::String("ludicrous".into()))
                )
                .is_err(),
            "values outside the plugin's choices are refused"
        );
        let codex = crate::agent_plugin("codex")
            .unwrap()
            .cli
            .structured_run
            .as_ref()
            .unwrap();
        let args =
            crate::agent_run::setting_arguments(codex, &store.agent_options(workspace, "codex"));
        assert!(args.contains(&"web_search=\"live\"".to_owned()), "{args:?}");
        assert!(
            args.contains(&"--ignore-user-config".to_owned()),
            "user config stays off by default"
        );
    }

    #[test]
    fn claude_code_events_become_thinking_and_search_steps() {
        let printed = concat!(
            r#"{"type":"system","subtype":"init"}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"WebSearch","input":{"query":"cube record 2026"}}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":""}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Checking the record\nmore"}]}}"#,
            "\n",
            r#"{"type":"result","structured_output":{}}"#,
        );
        assert_eq!(
            agent_steps("claude-code", printed),
            [
                AgentStep {
                    kind: "search".into(),
                    text: "cube record 2026".into()
                },
                AgentStep {
                    kind: "thinking".into(),
                    text: "Checking the record".into()
                },
            ]
        );
    }

    #[test]
    fn codex_progress_becomes_thinking_and_search_steps() {
        let printed = "user\nPrompt\n**Planning the hook**\nweb search: \nweb search: cube record 2026\nweb search: cube record 2026\ncodex\n{}";
        assert_eq!(
            agent_steps("codex", printed),
            [
                AgentStep {
                    kind: "thinking".into(),
                    text: "Planning the hook".into()
                },
                AgentStep {
                    kind: "search".into(),
                    text: "cube record 2026".into()
                },
            ]
        );
        assert!(agent_steps("unknown", printed).is_empty());
    }
}
