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

pub(crate) fn find_agent_cli(plugin: &AgentPlugin) -> Option<PathBuf> {
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
        let mut command = Command::new(executable);
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
        let mut process = command.spawn().map_err(|_| failed("it did not start"))?;
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
