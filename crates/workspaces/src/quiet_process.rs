//! Helper programs such as FFmpeg run without a window of their own. On Windows, a program
//! started from the desktop app would otherwise flash a console for every run.

use std::{ffi::OsStr, process::Command};

pub(crate) fn quiet_command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}
