//! Work folders for agents allowed to write files and run commands: one per video run, kept for
//! every stage so scripts and notes the agent makes stay available, and removed once the run is
//! finished.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::WorkspaceStore;

/// Added to the prompt when the agent works in a run's work folder.
pub(crate) const WORK_FOLDER_NOTE: &str = "You may write files and run commands in your \
working folder. It is kept for every step of this video, so scripts, notes and intermediate \
files you make now are still there in later steps. Keep your files inside it, and never change \
or delete files elsewhere.";

/// The folder beside the database that holds every run's work folder.
pub fn agent_work_root(database: &Path) -> PathBuf {
    let mut root = database.as_os_str().to_owned();
    root.push(".work");
    PathBuf::from(root)
}

impl WorkspaceStore {
    /// The run's work folder, created when missing. None for in-memory stores.
    pub(crate) fn agent_work_folder(&self, workspace_id: i64, run_id: i64) -> Option<PathBuf> {
        let database = self.0.path().filter(|path| !path.is_empty())?;
        let folder = agent_work_root(Path::new(database))
            .join(workspace_id.to_string())
            .join(run_id.to_string());
        fs::create_dir_all(&folder).ok()?;
        Some(folder)
    }

    /// Removes the work folders of runs that finished, failed, were cancelled or no longer
    /// exist. Returns how many were removed.
    pub fn remove_finished_agent_work(&self, database: &Path) -> io::Result<usize> {
        let root = agent_work_root(database);
        let Ok(workspaces) = fs::read_dir(&root) else {
            return Ok(0);
        };
        let mut removed = 0;
        for workspace in workspaces.flatten() {
            let Some(workspace_id) = number(&workspace.path()) else {
                continue;
            };
            for run in fs::read_dir(workspace.path())?.flatten() {
                let Some(run_id) = number(&run.path()) else {
                    continue;
                };
                let state: Option<String> = self
                    .0
                    .query_row(
                        "SELECT state FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
                        rusqlite::params![workspace_id, run_id],
                        |row| row.get(0),
                    )
                    .ok();
                if state
                    .as_deref()
                    .is_none_or(|state| matches!(state, "completed" | "failed" | "cancelled"))
                {
                    fs::remove_dir_all(run.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

fn number(path: &Path) -> Option<i64> {
    let name = path.file_name()?.to_str()?;
    name.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| name.parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftSettings, RunOptions};

    #[test]
    fn work_folders_last_for_the_run_and_go_when_it_ends() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Work".into(),
                    intent: "Test the work folder".into(),
                    profile_id: None,
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "work-folder", &RunOptions::default())
            .unwrap()
            .id;
        let folder = store.agent_work_folder(workspace, run).unwrap();
        fs::write(folder.join("script.py"), "print(42)").unwrap();
        assert_eq!(store.agent_work_folder(workspace, run).unwrap(), folder);
        let stale = agent_work_root(&database)
            .join(workspace.to_string())
            .join("999");
        fs::create_dir_all(&stale).unwrap();

        assert_eq!(store.remove_finished_agent_work(&database).unwrap(), 1);
        assert!(folder.join("script.py").is_file());
        assert!(!stale.exists());

        store
            .0
            .execute(
                "UPDATE generation_runs SET state = 'completed' WHERE id = ?1",
                [run],
            )
            .unwrap();
        assert_eq!(store.remove_finished_agent_work(&database).unwrap(), 1);
        assert!(!folder.exists());
    }
}
