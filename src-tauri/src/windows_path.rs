//! Installers add their folders to the PATH saved in the registry, which a running app doesn't
//! see. This reads the saved PATH again, so an agent's CLI installed while Arrumo is open is found.

use std::{env, ffi::OsString, os::windows::process::CommandExt, path::PathBuf, process::Command};

const KEYS: [&str; 2] = [
    r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    r"HKCU\Environment",
];

/// The system and user PATH from the registry, ahead of any folder of `current` they lack.
pub fn registry_path(current: OsString) -> Option<OsString> {
    let mut folders: Vec<PathBuf> = KEYS
        .iter()
        .filter_map(|key| saved_path(key))
        .flat_map(|value| {
            value
                .split(';')
                .map(|folder| PathBuf::from(expand(folder.trim())))
                .collect::<Vec<_>>()
        })
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

fn saved_path(key: &str) -> Option<String> {
    let output = Command::new("reg")
        .args(["query", key, "/v", "Path"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    parse(&String::from_utf8_lossy(&output.stdout))
}

/// The value from `reg query` output, such as `    Path    REG_EXPAND_SZ    C:\Tools;%USERPROFILE%\bin`.
fn parse(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut parts = line.trim().splitn(3, "    ");
        let name = parts.next()?;
        let kind = parts.next()?.trim();
        (name.eq_ignore_ascii_case("path") && kind.starts_with("REG_"))
            .then(|| parts.next().unwrap_or_default().trim().to_owned())
    })
}

/// Replaces `%NAME%` with the variable's value, leaving unknown names as they are.
fn expand(text: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match env::var(name) {
                    Ok(value) if !name.is_empty() => result.push_str(&value),
                    _ => result.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            None => {
                result.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_path_is_read_and_expanded() {
        let output = "\r\nHKEY_CURRENT_USER\\Environment\r\n    Path    REG_EXPAND_SZ    %SystemRoot%\\bin;C:\\Tools\r\n\r\n";
        let value = parse(output).unwrap();
        assert_eq!(value, "%SystemRoot%\\bin;C:\\Tools");
        let root = env::var("SystemRoot").unwrap();
        assert_eq!(expand(&value), format!("{root}\\bin;C:\\Tools"));
        assert_eq!(expand("%ARRUMO_NOT_SET%\\x"), "%ARRUMO_NOT_SET%\\x");
    }
}
