//! Where Arrumo finds FFmpeg and ffprobe. Installed builds carry their own copies, named
//! `arrumo-ffmpeg` and `arrumo-ffprobe` so they never collide with a system FFmpeg, next to the
//! executable. `ARRUMO_FFMPEG` and `ARRUMO_FFPROBE` override them; development builds fall back to
//! the tools on the PATH.

use serde::Serialize;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
};

pub fn ffmpeg() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        resolve(
            "ffmpeg",
            env::var_os("ARRUMO_FFMPEG"),
            env::current_exe().ok().as_deref(),
        )
    })
}

pub fn ffprobe() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        resolve(
            "ffprobe",
            env::var_os("ARRUMO_FFPROBE"),
            env::current_exe().ok().as_deref(),
        )
    })
}

fn resolve(name: &str, override_path: Option<OsString>, executable: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path.filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    executable
        .and_then(Path::parent)
        .map(|directory| directory.join(format!("arrumo-{name}{}", env::consts::EXE_SUFFIX)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolsStatus {
    pub ffmpeg: bool,
    pub ffprobe: bool,
}

/// Whether FFmpeg and ffprobe start. Every preview, render and export needs both.
pub fn media_tools_status() -> MediaToolsStatus {
    let runs = |path: &Path| {
        crate::quiet_command(path)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    MediaToolsStatus {
        ffmpeg: runs(ffmpeg()),
        ffprobe: runs(ffprobe()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_tools_next_to_the_executable_win_over_the_path() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("arrumo");
        assert_eq!(
            resolve("ffprobe", None, Some(&executable)),
            PathBuf::from("ffprobe")
        );
        let bundled = temp
            .path()
            .join(format!("arrumo-ffprobe{}", env::consts::EXE_SUFFIX));
        std::fs::write(&bundled, b"").unwrap();
        assert_eq!(resolve("ffprobe", None, Some(&executable)), bundled);
        assert_eq!(resolve("ffprobe", None, None), PathBuf::from("ffprobe"));
        let chosen = PathBuf::from("/opt/ffprobe");
        assert_eq!(
            resolve("ffprobe", Some(chosen.clone().into()), Some(&executable)),
            chosen
        );
    }
}
