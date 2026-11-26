//! Apps opened from the Dock, Finder or a desktop launcher get a minimal PATH, not the one the
//! user's terminal has, so agent CLIs installed with Homebrew, npm or into `~/.local/bin` (and the
//! `node` they run on) would not be found. On macOS and Linux this asks the user's login shell for
//! its PATH and puts those folders first.

use std::{
    env,
    ffi::OsString,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
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
    shell_path(env::var_os("PATH").unwrap_or_default())
}

/// The login shell's PATH again, ahead of `current`, such as after an agent's CLI was installed
/// while Arrumo was open.
pub fn shell_path(current: OsString) -> Option<OsString> {
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
    // A file, not a pipe: helpers that start-up files leave running inherit the output, and
    // reading a pipe would wait for all of them to exit, keeping the app from ever opening.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let output_file =
        env::temp_dir().join(format!("arrumo-shell-path-{}-{stamp}", std::process::id()));
    let output = read_shell_path(shell, &output_file);
    let _ = fs::remove_file(&output_file);
    merge(&output?, current)
}

fn read_shell_path(shell: PathBuf, output_file: &Path) -> Option<String> {
    let mut child = Command::new(shell)
        .args([
            "-ilc",
            &format!("printf '\\n{MARKER}%s{MARKER}\\n' \"$PATH\""),
        ])
        .stdin(Stdio::null())
        .stdout(fs::File::create(output_file).ok()?)
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
    fs::read_to_string(output_file).ok()
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

#[cfg(test)]
mod lingering_helpers {
    use super::*;

    #[test]
    fn a_helper_left_running_by_the_shell_does_not_delay_the_path() {
        let folder = env::temp_dir().join(format!("arrumo-shell-test-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let shell = folder.join("shell");
        // Like start-up files that launch a background agent without redirecting its output.
        fs::write(&shell, "#!/bin/sh\nsleep 20 &\neval \"$2\"\n").unwrap();
        fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let started = Instant::now();
        let output = read_shell_path(shell, &folder.join("output")).unwrap();
        assert!(started.elapsed() < LIMIT, "took {:?}", started.elapsed());
        assert!(output.contains(MARKER));
        let _ = fs::remove_dir_all(folder);
    }
}
