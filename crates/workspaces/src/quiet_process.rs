//! Helper programs such as FFmpeg run without a window of their own. On Windows, a program
//! started from the desktop app would otherwise flash a console for every run.

use std::{
    error::Error,
    ffi::OsStr,
    fmt, io,
    panic::Location,
    path::Path,
    process::{Child, Command, ExitStatus, Output},
};

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

/// Runs a command like `spawn`, `output` and `status`, but a failure names the program and the
/// source line that started it: a bare "No such file or directory" cannot say which was missing.
pub(crate) trait Launch {
    fn launch(&mut self) -> io::Result<Child>;
    fn launch_output(&mut self) -> io::Result<Output>;
    fn launch_status(&mut self) -> io::Result<ExitStatus>;
}

impl Launch for Command {
    #[track_caller]
    fn launch(&mut self) -> io::Result<Child> {
        let at = Location::caller();
        self.spawn().map_err(|error| not_started(self, error, at))
    }

    #[track_caller]
    fn launch_output(&mut self) -> io::Result<Output> {
        let at = Location::caller();
        let timing = time_program(self);
        let output = self.output().map_err(|error| not_started(self, error, at));
        timing.finish(output.as_ref().is_ok_and(|output| output.status.success()));
        output
    }

    #[track_caller]
    fn launch_status(&mut self) -> io::Result<ExitStatus> {
        let at = Location::caller();
        let timing = time_program(self);
        let status = self.status().map_err(|error| not_started(self, error, at));
        timing.finish(status.as_ref().is_ok_and(ExitStatus::success));
        status
    }
}

/// The program's file name, never its folder or arguments.
fn program_name(command: &Command) -> String {
    Path::new(command.get_program())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn time_program(command: &Command) -> crate::telemetry::Timing {
    crate::telemetry::time_part("process", &program_name(command), &[])
}

fn not_started(command: &Command, error: io::Error, at: &Location<'_>) -> io::Error {
    let program = program_name(command);
    io::Error::new(
        error.kind(),
        format!("Could not run {program} ({}): {error}", place(at)),
    )
}

/// `file.rs:line`, without the folders, which reports would redact as a path.
pub(crate) fn place(at: &Location<'_>) -> String {
    let file = at.file().rsplit(['/', '\\']).next().unwrap_or_default();
    format!("{file}:{}", at.line())
}

/// An error with the step that failed and the source line it failed at, keeping the original
/// error as its cause.
#[derive(Debug)]
pub(crate) struct StepError {
    step: &'static str,
    at: String,
    source: Box<dyn Error>,
}

impl fmt::Display for StepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.step, self.at)
    }
}

impl Error for StepError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

/// Names the step a fallible call belongs to, such as `.step("Rendering the storyboard")`.
pub(crate) trait Step<T> {
    fn step(self, step: &'static str) -> Result<T, StepError>;
}

impl<T, E: Into<Box<dyn Error>>> Step<T> for Result<T, E> {
    #[track_caller]
    fn step(self, step: &'static str) -> Result<T, StepError> {
        let at = place(Location::caller());
        self.map_err(|error| StepError {
            step,
            at,
            source: error.into(),
        })
    }
}

/// [`Step`] for functions that return `io::Result`: the step and line lead the message and the
/// kind is kept.
pub(crate) trait IoStep<T> {
    fn io_step(self, step: &'static str) -> io::Result<T>;
}

impl<T> IoStep<T> for io::Result<T> {
    #[track_caller]
    fn io_step(self, step: &'static str) -> io::Result<T> {
        let at = place(Location::caller());
        self.map_err(|error| io::Error::new(error.kind(), format!("{step} ({at}): {error}")))
    }
}

/// An error and each error that caused it, outermost first.
pub(crate) fn error_sources<'a>(
    error: &'a (dyn Error + 'static),
) -> impl Iterator<Item = &'a (dyn Error + 'static)> + Clone {
    std::iter::successors(Some(error), |&error: &&'a (dyn Error + 'static)| {
        error.source()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_that_cannot_start_is_named_with_where_it_was_started() {
        let error = quiet_command("arrumo-no-such-program")
            .launch_output()
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let text = error.to_string();
        assert!(
            text.starts_with("Could not run arrumo-no-such-program (quiet_process.rs:"),
            "{text}"
        );
        let reported = crate::telemetry::redact_lines(&text);
        assert!(reported.contains("arrumo-no-such-program"), "{reported}");
        assert!(reported.contains("quiet_process.rs:"), "{reported}");
    }

    #[test]
    fn an_agent_failure_inside_a_step_can_still_be_found() {
        let failed: Result<(), crate::AgentRunError> = Err(crate::AgentRunError::TimedOut);
        let error: Box<dyn Error> = failed.step("Reviewing the scenes").unwrap_err().into();
        let found = error_sources(&*error)
            .find_map(|error| error.downcast_ref::<crate::AgentRunError>())
            .map(ToString::to_string);
        assert_eq!(found.as_deref(), Some("The agent did not answer in time"));
    }

    #[test]
    fn a_step_keeps_its_cause_and_reports_without_folders() {
        let missing: io::Result<()> = Err(io::Error::from(io::ErrorKind::NotFound));
        let error: Box<dyn Error> = missing.step("Copying the fonts").unwrap_err().into();
        let chain = crate::error_chain(&*error);
        let mut lines = chain.lines();
        let first = lines.next().unwrap();
        assert!(
            first.starts_with("Copying the fonts (quiet_process.rs:"),
            "{chain}"
        );
        assert!(lines.next().is_some(), "{chain}");
        let reported = crate::telemetry::redact_lines(&chain);
        assert!(
            reported.starts_with("Copying the fonts (quiet_process.rs:"),
            "{reported}"
        );
        // Paths in a cause are still cut from reports.
        let with_path =
            crate::telemetry::redact_lines("Could not open /Users/someone/Movies/a.mp4");
        assert!(!with_path.contains("someone"), "{with_path}");
    }
}
