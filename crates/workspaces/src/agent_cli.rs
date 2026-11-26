use crate::Launch;
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::RwLock,
    thread,
    time::{Duration, Instant},
};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{AgentPlugin, WorkspaceStore, agent_plugin};

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoginStatus {
    Confirmed,
    Unconfirmed,
    Unavailable,
    TimedOut,
    Failed,
}

pub fn agent_cli_detected(provider: &str) -> bool {
    agent_cli_path(provider).is_some()
}

impl WorkspaceStore {
    /// The program chosen for an agent, if any.
    pub fn agent_executable(&self, provider: &str) -> rusqlite::Result<Option<String>> {
        self.0
            .query_row(
                "SELECT path FROM agent_executables WHERE plugin_id = ?1",
                [provider],
                |row| row.get(0),
            )
            .optional()
    }

    /// Chooses the program that runs an agent, or with `None` goes back to the PATH. Only the
    /// desktop's settings call this: agents talk to the MCP and A2A servers, and must not pick
    /// the program the next run executes.
    pub fn set_agent_executable(
        &self,
        provider: &str,
        path: Option<&Path>,
    ) -> Result<(), AgentExecutableError> {
        let plugin = agent_plugin(provider).ok_or(AgentExecutableError::UnknownAgent)?;
        match path {
            Some(path) => {
                if !path.is_absolute() || !runnable(path) {
                    return Err(AgentExecutableError::NotRunnable);
                }
                let text = path.to_str().ok_or(AgentExecutableError::NotRunnable)?;
                self.0.execute(
                    "INSERT INTO agent_executables (plugin_id, path) VALUES (?1, ?2)
                     ON CONFLICT (plugin_id) DO UPDATE SET path = excluded.path",
                    params![plugin.id, text],
                )?;
            }
            None => {
                self.0.execute(
                    "DELETE FROM agent_executables WHERE plugin_id = ?1",
                    [&plugin.id],
                )?;
                CHOSEN
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&plugin.id);
            }
        }
        load_executables(&self.0)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum AgentExecutableError {
    UnknownAgent,
    NotRunnable,
    Database(rusqlite::Error),
}

impl From<rusqlite::Error> for AgentExecutableError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

pub fn check_agent_login(provider: &str) -> AgentLoginStatus {
    let Some(plugin) = agent_plugin(provider) else {
        return AgentLoginStatus::Unavailable;
    };
    let Some(executable) = find_agent_cli(plugin) else {
        return AgentLoginStatus::Unavailable;
    };
    let Ok(directory) = tempfile::tempdir() else {
        return AgentLoginStatus::Failed;
    };
    let mut command = agent_command(executable);
    command
        .args(&plugin.cli.auth_status_args)
        .current_dir(directory.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let Ok(mut process) = command.launch() else {
        return AgentLoginStatus::Failed;
    };

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match process.try_wait() {
            Ok(Some(status)) if status.success() => return AgentLoginStatus::Confirmed,
            Ok(Some(status)) if status.code() == Some(1) => return AgentLoginStatus::Unconfirmed,
            Ok(Some(_)) => return AgentLoginStatus::Failed,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            result => {
                let _ = process.kill();
                let _ = process.wait();
                return if result.is_ok() {
                    AgentLoginStatus::TimedOut
                } else {
                    AgentLoginStatus::Failed
                };
            }
        }
    }
}

/// Agent CLIs people chose by hand because they aren't on the PATH, by plugin id. Every process
/// loads them when it opens the database, so the desktop, CLI and MCP server resolve alike.
static CHOSEN: RwLock<BTreeMap<String, PathBuf>> = RwLock::new(BTreeMap::new());

/// A PATH read again after Arrumo started, so a CLI installed meanwhile is found without a
/// restart. Arrumo's own PATH can't change safely once its threads are running.
static SEARCH_PATH: RwLock<Option<OsString>> = RwLock::new(None);

/// Finds agents, and runs them, with `path` from now on.
pub fn set_agent_search_path(path: OsString) {
    *SEARCH_PATH
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(path);
}

/// The PATH agents are found and run with.
pub fn agent_search_path() -> Option<OsString> {
    SEARCH_PATH
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .or_else(|| env::var_os("PATH"))
}

/// Starts an agent's program with the PATH it was found on, which also finds the runtime it needs,
/// such as `node`.
pub(crate) fn agent_command(executable: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(executable);
    if let Some(path) = SEARCH_PATH
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        command.env("PATH", path);
    }
    command
}

pub(crate) fn load_executables(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare("SELECT plugin_id, path FROM agent_executables")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    CHOSEN
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .extend(rows);
    Ok(())
}

fn runnable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Where an agent's CLI is: the program chosen in settings while it still exists, otherwise the
/// first match on the PATH.
pub fn agent_cli_path(provider: &str) -> Option<PathBuf> {
    agent_plugin(provider).and_then(find_agent_cli)
}

pub(crate) fn find_agent_cli(plugin: &AgentPlugin) -> Option<PathBuf> {
    let chosen = CHOSEN
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&plugin.id)
        .cloned();
    if let Some(path) = chosen.filter(|path| runnable(path)) {
        return Some(path);
    }
    let home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    search_folders(agent_search_path(), home)
        .into_iter()
        .find_map(|dir| program_in(&dir, &plugin.cli.executable))
}

/// The PATH's folders, then the per-user program folder, `~/.local/bin`, where official agent
/// installers put their program without always adding it to the PATH.
fn search_folders(search_path: Option<OsString>, home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = search_path
        .iter()
        .flat_map(env::split_paths)
        .filter(|dir| dir.is_absolute())
        .collect();
    if let Some(user_programs) = home
        .filter(|home| home.is_absolute())
        .map(|home| home.join(".local").join("bin"))
        .filter(|dir| !folders.contains(dir))
    {
        folders.push(user_programs);
    }
    folders
}

fn program_in(dir: &Path, executable: &str) -> Option<PathBuf> {
    let candidate = dir.join(executable);
    // npm installs CLIs on Windows as `.cmd` shims, so every PATHEXT extension counts.
    #[cfg(windows)]
    {
        let extensions = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        extensions
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| candidate.with_extension(extension.trim_start_matches('.')))
            .find(|candidate| candidate.is_file())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = candidate.metadata().ok()?;
        (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(candidate)
    }
}

/// Used percent, reset time in Unix seconds and window length in minutes.
pub(crate) type AgentUsage = (f64, Option<i64>, Option<i64>);

/// The share used of an agent's tightest allowance window, read through its `usageRun`:
/// `(used percent, reset time in Unix seconds, window length in minutes)`. `None` when the plugin
/// describes no usage read or its CLI is not installed.
pub(crate) fn read_agent_usage(plugin: &AgentPlugin) -> Option<std::io::Result<AgentUsage>> {
    use std::io::{BufRead, BufReader, Read, Write};

    let run = plugin.cli.usage_run.as_ref()?;
    let executable = find_agent_cli(plugin)?;
    Some((|| {
        let failed = |what: &str| {
            std::io::Error::other(format!("The agent's usage could not be read: {what}"))
        };
        let directory = tempfile::tempdir()?;
        let mut command = agent_command(executable);
        command
            .args(&run.args)
            .current_dir(directory.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut process = command.launch().map_err(|_| failed("it did not start"))?;
        let stdout = process.stdout.take().ok_or_else(|| failed("no output"))?;
        let (sender, receiver) = std::sync::mpsc::channel();
        let wanted = run.response_id.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout).take(4 * 1024 * 1024);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 0) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line)
                    && value.get("id") == Some(&wanted)
                {
                    let _ = sender.send(value);
                    return;
                }
                line.clear();
            }
        });
        // Kept open until the answer arrives: some agents stop once their input closes.
        let mut stdin = process.stdin.take();
        if let Some(stdin) = stdin.as_mut() {
            for message in &run.input {
                let _ = writeln!(stdin, "{message}");
            }
            let _ = stdin.flush();
        }
        let reply = receiver.recv_timeout(Duration::from_secs(20));
        drop(stdin);
        let _ = process.kill();
        let _ = process.wait();
        let reply = reply.map_err(|_| failed("no answer in time"))?;
        let number = |pointer: &str| reply.pointer(pointer).and_then(serde_json::Value::as_f64);
        run.windows
            .iter()
            .filter_map(|window| {
                let used = number(&window.used_percent).filter(|used| used.is_finite())?;
                Some((
                    used.clamp(0.0, 100.0),
                    window
                        .resets_at
                        .as_deref()
                        .and_then(|at| reply.pointer(at)?.as_i64()),
                    window
                        .minutes
                        .as_deref()
                        .and_then(|at| reply.pointer(at)?.as_i64()),
                ))
            })
            .max_by(|first, second| first.0.total_cmp(&second.0))
            .ok_or_else(|| failed("the answer has no usage"))
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_program_folder_is_searched_after_the_path() {
        let home = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let user_programs = home.path().join(".local").join("bin");
        let path = env::join_paths([other.path()]).unwrap();
        assert_eq!(
            search_folders(Some(path.clone()), Some(home.path().to_path_buf())),
            [other.path().to_path_buf(), user_programs.clone()]
        );
        // Already on the PATH, it keeps its place and isn't searched twice.
        let both = env::join_paths([user_programs.as_path(), other.path()]).unwrap();
        assert_eq!(
            search_folders(Some(both), Some(home.path().to_path_buf())),
            [user_programs.clone(), other.path().to_path_buf()]
        );
        // No PATH at all still finds it; a relative home is ignored.
        assert_eq!(
            search_folders(None, Some(home.path().to_path_buf())),
            std::slice::from_ref(&user_programs)
        );
        assert!(search_folders(None, Some(PathBuf::from("relative"))).is_empty());

        std::fs::create_dir_all(&user_programs).unwrap();
        let name = if cfg!(windows) { "agent.exe" } else { "agent" };
        let program = user_programs.join(name);
        std::fs::write(&program, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let found = search_folders(Some(path), Some(home.path().to_path_buf()))
            .into_iter()
            .find_map(|dir| program_in(&dir, "agent"));
        // Windows tries each PATHEXT extension, which may differ in case from the file's.
        assert_eq!(found.as_deref().and_then(Path::parent), program.parent());
    }

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn a_chosen_program_is_used_while_it_exists_and_left_out_of_backups() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::open(temp.path().join("workspaces.sqlite")).unwrap();
        let plugin = &crate::agent_plugins()[0];
        let program = temp
            .path()
            .join(format!("agent{}", env::consts::EXE_SUFFIX));
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let not_runnable = temp.path().join("notes.txt");
            std::fs::write(&not_runnable, "").unwrap();
            assert!(matches!(
                store.set_agent_executable(&plugin.id, Some(&not_runnable)),
                Err(AgentExecutableError::NotRunnable)
            ));
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(matches!(
            store.set_agent_executable(&plugin.id, Some(Path::new("relative/agent"))),
            Err(AgentExecutableError::NotRunnable)
        ));
        assert!(matches!(
            store.set_agent_executable("not-an-agent", Some(&program)),
            Err(AgentExecutableError::UnknownAgent)
        ));

        store
            .set_agent_executable(&plugin.id, Some(&program))
            .unwrap();
        assert_eq!(agent_cli_path(&plugin.id), Some(program.clone()));
        assert_eq!(
            store.agent_executable(&plugin.id).unwrap().as_deref(),
            program.to_str()
        );

        let backup = temp.path().join("backup.zip");
        store
            .export_backup(&temp.path().join("workspaces.sqlite"), &backup)
            .unwrap();
        let staging = tempfile::tempdir().unwrap();
        zip::ZipArchive::new(std::fs::File::open(&backup).unwrap())
            .unwrap()
            .extract(staging.path())
            .unwrap();
        let snapshot =
            rusqlite::Connection::open(staging.path().join("database/workspaces.sqlite")).unwrap();
        let saved: i64 = snapshot
            .query_row("SELECT count(*) FROM agent_executables", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(saved, 0);

        std::fs::remove_file(&program).unwrap();
        assert_ne!(agent_cli_path(&plugin.id), Some(program.clone()));
        store.set_agent_executable(&plugin.id, None).unwrap();
        assert_eq!(store.agent_executable(&plugin.id).unwrap(), None);
    }
}
