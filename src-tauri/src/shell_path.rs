//! Apps opened from the Dock, Finder or a desktop launcher get a minimal PATH, not the one the
//! user's terminal has, so agent CLIs installed with Homebrew, npm or into `~/.local/bin` (and the
//! `node` they run on) would not be found. On macOS and Linux this asks the user's login shell for
//! its PATH and puts those folders first.

use std::{
    env,
    ffi::OsString,
    io::{IsTerminal, Read},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MARKER: &str = "__ARRUMO_PATH__";
/// Interactive shell start-up files can be slow; the app opens without them after this.
const LIMIT: Duration = Duration::from_secs(3);

/// The PATH to use, or `None` when it should stay as it is: launched from a terminal, which
/// already passed the user's PATH, or the shell gave nothing usable in time.
pub fn login_shell_path() -> Option<OsString> {
    if std::io::stdin().is_terminal() {
        return None;
    }
    let shell = env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|shell| shell.is_absolute() && shell.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(if cfg!(target_os = "macos") {
                "/bin/zsh"
            } else {
                "/bin/sh"
            })
        });
    let mut child = Command::new(shell)
        .args([
            "-ilc",
            &format!("printf '\\n{MARKER}%s{MARKER}\\n' \"$PATH\""),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + LIMIT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    merge(&output, env::var_os("PATH").unwrap_or_default())
}

/// The shell's folders first, then any of the current ones it lacks. `None` when the output has
/// no PATH between the markers, since start-up files may print other text around it.
fn merge(output: &str, current: OsString) -> Option<OsString> {
    let start = output.find(MARKER)? + MARKER.len();
    let end = start + output[start..].find(MARKER)?;
    let shell = &output[start..end];
    let mut folders: Vec<PathBuf> = env::split_paths(shell)
        .filter(|folder| folder.is_absolute())
        .collect();
    if folders.is_empty() {
        return None;
    }
    for folder in env::split_paths(&current) {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    env::join_paths(folders).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_login_shell_path_comes_first_and_keeps_the_rest() {
        let output = format!("Welcome!\n{MARKER}/opt/homebrew/bin:/usr/bin:relative{MARKER}\n");
        let merged = merge(&output, OsString::from("/usr/bin:/bin")).unwrap();
        assert_eq!(merged, OsString::from("/opt/homebrew/bin:/usr/bin:/bin"));
    }

    #[test]
    fn output_without_a_path_changes_nothing() {
        assert_eq!(merge("no markers here", OsString::from("/usr/bin")), None);
        assert_eq!(
            merge(&format!("{MARKER}{MARKER}"), OsString::from("/usr/bin")),
            None
        );
    }
}
