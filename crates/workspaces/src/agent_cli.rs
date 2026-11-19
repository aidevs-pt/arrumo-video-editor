use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{AgentPlugin, agent_plugin};

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
    agent_plugin(provider).and_then(find_agent_cli).is_some()
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
    let mut command = Command::new(executable);
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
    let Ok(mut process) = command.spawn() else {
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

fn find_agent_cli(plugin: &AgentPlugin) -> Option<PathBuf> {
    let executable = &plugin.cli.executable;
    let search_path = env::var_os("PATH")?;
    env::split_paths(&search_path)
        .filter(|dir| dir.is_absolute())
        .find_map(|dir| {
            let candidate = dir.join(executable);
            #[cfg(windows)]
            {
                let candidate = candidate.with_extension("exe");
                candidate.is_file().then_some(candidate)
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = candidate.metadata().ok()?;
                (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
                    .then_some(candidate)
            }
        })
}
