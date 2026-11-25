//! Agent sessions Arrumo kept so a follow-up could continue them. They stay in the agent's own
//! history until nothing can need them: when their video run has completed, failed or been
//! cancelled (or deleted), or right after the request when no run owns them. Where an agent
//! keeps its session files is declared by its plugin (`structuredRun.resume.cleanup`).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use rusqlite::params;

use crate::{
    AgentPlugin, WorkspaceStore, agent_logs::AgentLogScope, plugin_catalog::SessionCleanup,
};

/// Sessions without a run that outlive this were left behind by an interrupted request.
const ORPHAN_SECONDS: i64 = 6 * 60 * 60;
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 50_000;

impl WorkspaceStore {
    pub(crate) fn record_agent_session(
        &self,
        scope: AgentLogScope<'_>,
        provider: &str,
        session: &str,
        round: Option<i64>,
    ) {
        let _ = self.0.execute(
            "INSERT OR IGNORE INTO agent_sessions (provider, session, workspace_id, run_id, stage, round)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                provider,
                session,
                scope.workspace_id,
                scope.run_id,
                scope.stage,
                round
            ],
        );
    }

    /// The run's current round: 1, plus one for every review where the creator sent feedback
    /// (asked for changes, rejected, or approved with notes).
    pub(crate) fn run_round(&self, run_id: i64) -> rusqlite::Result<i64> {
        self.0.query_row(
            "SELECT 1 + count(*) FROM production_results
             WHERE run_id = ?1 AND (decision IN ('revise', 'rejected')
                OR (decision = 'approved' AND trim(coalesce(review_notes, '')) != ''))",
            [run_id],
            |row| row.get(0),
        )
    }

    /// The session the run's agent has been using this round, if one was kept.
    pub(crate) fn round_session(&self, run_id: i64, provider: &str, round: i64) -> Option<String> {
        self.0
            .query_row(
                "SELECT session FROM agent_sessions WHERE run_id = ?1 AND provider = ?2 AND round = ?3
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                params![run_id, provider, round],
                |row| row.get(0),
            )
            .ok()
    }

    /// Stops offering a session that could not be continued; it is still removed with the run.
    pub(crate) fn forget_round_session(&self, provider: &str, session: &str) {
        let _ = self.0.execute(
            "UPDATE agent_sessions SET round = NULL WHERE provider = ?1 AND session = ?2",
            params![provider, session],
        );
    }

    /// Removes the sessions a request without a run kept, once that request is over.
    pub(crate) fn release_agent_sessions(&self, scope: AgentLogScope<'_>) {
        if scope.run_id.is_some() {
            return;
        }
        let _ = self.remove_agent_sessions(
            "SELECT provider, session FROM agent_sessions
             WHERE workspace_id = ?1 AND run_id IS NULL AND stage = ?2",
            params![scope.workspace_id, scope.stage],
        );
    }

    /// Removes every kept session that nothing can need any more. Returns how many were removed.
    pub fn sweep_agent_sessions(&self) -> rusqlite::Result<usize> {
        self.remove_agent_sessions(
            "SELECT session.provider, session.session FROM agent_sessions session
             LEFT JOIN generation_runs run ON run.id = session.run_id
             WHERE (session.run_id IS NOT NULL
                    AND (run.id IS NULL OR run.state IN ('completed', 'failed', 'cancelled')))
                OR (session.run_id IS NULL AND session.created_at < unixepoch() - ?1)",
            params![ORPHAN_SECONDS],
        )
    }

    fn remove_agent_sessions(
        &self,
        query: &str,
        parameters: impl rusqlite::Params,
    ) -> rusqlite::Result<usize> {
        let sessions = self
            .0
            .prepare(query)?
            .query_map(parameters, |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut removed = 0;
        for (provider, session) in sessions {
            let cleanup = crate::agent_plugin(&provider).and_then(session_cleanup);
            // Keep the record to retry later only when the files are there but could not go.
            if cleanup.is_some_and(|cleanup| remove_session_files(cleanup, &session).is_err()) {
                continue;
            }
            removed += self.0.execute(
                "DELETE FROM agent_sessions WHERE provider = ?1 AND session = ?2",
                [&provider, &session],
            )?;
        }
        Ok(removed)
    }
}

fn session_cleanup(plugin: &AgentPlugin) -> Option<&SessionCleanup> {
    plugin
        .cli
        .structured_run
        .as_ref()?
        .resume
        .as_ref()?
        .cleanup
        .as_ref()
}

fn agent_home(cleanup: &SessionCleanup) -> Option<PathBuf> {
    let configured = cleanup
        .home_env
        .as_deref()
        .and_then(std::env::var_os)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    configured.or_else(|| {
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(|home| PathBuf::from(home).join(&cleanup.home))
    })
}

/// Deletes the agent's files for `session` under its declared session folder, without following
/// links, then any folders that removal left empty.
pub(crate) fn remove_session_files(cleanup: &SessionCleanup, session: &str) -> io::Result<()> {
    // Only IDs long and plain enough that a file-name suffix cannot match anything else.
    if session.len() < 16
        || session.len() > 128
        || !session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Ok(());
    }
    let Some(home) = agent_home(cleanup) else {
        return Ok(());
    };
    let root = home.join(&cleanup.directory);
    let suffix = cleanup.file_suffix.replace("{session}", session);
    let mut matches = Vec::new();
    let mut seen = 0;
    collect(&root, &suffix, 0, &mut seen, &mut matches)?;
    for file in matches {
        match fs::remove_file(&file) {
            Ok(()) => remove_empty_parents(&file, &root),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn collect(
    folder: &Path,
    suffix: &str,
    depth: usize,
    seen: &mut usize,
    matches: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        *seen += 1;
        if *seen > MAX_ENTRIES {
            return Ok(());
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() && depth < MAX_DEPTH {
            collect(&path, suffix, depth + 1, seen, matches)?;
        } else if kind.is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(suffix))
        {
            matches.push(path);
        }
    }
    Ok(())
}

fn remove_empty_parents(file: &Path, root: &Path) {
    let mut folder = file.parent();
    while let Some(current) = folder {
        if current == root || !current.starts_with(root) || fs::remove_dir(current).is_err() {
            break;
        }
        folder = current.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_the_sessions_files_and_empty_folders() {
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join("sessions");
        let day = sessions.join("2026/11/22");
        fs::create_dir_all(&day).unwrap();
        let id = "01a1f858-ea8b-74c3-ac1e-740ffe6ef118";
        fs::write(
            day.join(format!("rollout-2026-11-22T09-45-39-{id}.jsonl")),
            "x",
        )
        .unwrap();
        fs::write(sessions.join("other-01a1f859-07c8-77b3-a45b.jsonl"), "keep").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join(format!("{id}.jsonl")), "keep").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), sessions.join("link")).unwrap();

        // Each test uses its own variable, so parallel tests do not see it.
        unsafe { std::env::set_var("ARRUMO_TEST_AGENT_HOME", home.path()) };
        let cleanup = SessionCleanup {
            home_env: Some("ARRUMO_TEST_AGENT_HOME".into()),
            home: ".agent".into(),
            directory: "sessions".into(),
            file_suffix: "{session}.jsonl".into(),
        };
        remove_session_files(&cleanup, id).unwrap();
        assert!(!sessions.join("2026").exists(), "empty date folders go too");
        assert!(
            sessions
                .join("other-01a1f859-07c8-77b3-a45b.jsonl")
                .exists()
        );
        assert!(
            outside.path().join(format!("{id}.jsonl")).exists(),
            "links are not followed"
        );
        // Short or unusual IDs are never used to match files.
        fs::write(sessions.join("ab.jsonl"), "keep").unwrap();
        remove_session_files(&cleanup, "ab").unwrap();
        assert!(sessions.join("ab.jsonl").exists());
    }

    #[test]
    fn a_round_lasts_until_the_creator_sends_feedback() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                crate::DraftSettings {
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
                &crate::RunOptions::default(),
            )
            .unwrap()
            .id;
        let scope = AgentLogScope {
            workspace_id: workspace,
            run_id: Some(run),
            stage: "script",
            attempt: Some(1),
        };
        assert_eq!(store.run_round(run).unwrap(), 1);
        assert_eq!(store.round_session(run, "claude-code", 1), None);
        store.record_agent_session(scope, "claude-code", "first-session", Some(1));
        assert_eq!(
            store.round_session(run, "claude-code", 1).as_deref(),
            Some("first-session")
        );
        assert_eq!(store.round_session(run, "codex", 1), None);
        let review = |decision: &str, notes: &str| {
            store
                .0
                .execute(
                    "INSERT INTO production_results (run_id, attempt, stage_index, result_json, decision, review_notes)
                     VALUES (?1, 1, 0, '{}', ?2, ?3)",
                    params![run, decision, notes],
                )
                .unwrap();
        };
        // Approving without notes keeps the round; feedback of any kind starts the next one.
        review("approved", "  ");
        assert_eq!(store.run_round(run).unwrap(), 1);
        review("approved", "Shorter intro");
        assert_eq!(store.run_round(run).unwrap(), 2);
        review("revise", "");
        review("rejected", "");
        assert_eq!(store.run_round(run).unwrap(), 4);
        assert_eq!(store.round_session(run, "claude-code", 4), None);
        // A session that could not be continued is no longer offered.
        store.forget_round_session("claude-code", "first-session");
        assert_eq!(store.round_session(run, "claude-code", 1), None);
    }

    #[test]
    fn sessions_go_when_their_run_is_over_or_they_have_no_run() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let scope = |run_id, stage| AgentLogScope {
            workspace_id: 1,
            run_id,
            stage,
            attempt: None,
        };
        store.record_agent_session(
            scope(None, "context"),
            "codex",
            "a".repeat(20).as_str(),
            None,
        );
        store.record_agent_session(
            scope(Some(99), "script"),
            "codex",
            "b".repeat(20).as_str(),
            Some(1),
        );
        store.release_agent_sessions(scope(Some(99), "script"));
        let count = |store: &WorkspaceStore| -> i64 {
            store
                .0
                .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(count(&store), 2, "a run's sessions wait for the run");
        store.release_agent_sessions(scope(None, "context"));
        assert_eq!(count(&store), 1);
        // Run 99 does not exist, so nothing can need its session.
        assert_eq!(store.sweep_agent_sessions().unwrap(), 1);
        assert_eq!(count(&store), 0);
    }
}
