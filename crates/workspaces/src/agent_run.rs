//! Runs an agent provider's declared structured run: one non-interactive CLI call that answers a
//! prompt with JSON following a schema. Provider flags come from the plugin manifest.

use std::{
    error::Error,
    fmt, fs,
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    AgentPlugin,
    agent_cli::find_agent_cli,
    plugin_catalog::{StructuredRun, StructuredSource},
};

const MAX_ANSWER_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum AgentRunError {
    /// The provider does not declare structured runs.
    Unsupported,
    /// The provider's CLI is not installed or not on the PATH.
    Unavailable,
    TimedOut,
    /// The agent exited with an error, reported failure, or returned something unreadable.
    Failed(&'static str),
}

impl fmt::Display for AgentRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "The selected agent cannot run production stages",
            Self::Unavailable => "The selected agent's command line tool was not found",
            Self::TimedOut => "The agent did not answer in time",
            Self::Failed(reason) => reason,
        })
    }
}

impl Error for AgentRunError {}

/// Runs the agent once. `tick` is called about every second while it works, so callers can renew
/// leases or stop early by returning an error.
pub(crate) fn run_structured(
    plugin: &AgentPlugin,
    prompt: &str,
    schema: &Value,
    timeout: Duration,
    mut tick: impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<Value, AgentRunError> {
    let run = plugin
        .cli
        .structured_run
        .as_ref()
        .ok_or(AgentRunError::Unsupported)?;
    let executable = find_agent_cli(plugin).ok_or(AgentRunError::Unavailable)?;
    let failed = |reason| AgentRunError::Failed(reason);
    let directory = tempfile::tempdir().map_err(|_| failed("Could not prepare the agent run"))?;
    let schema_text = schema.to_string();
    let schema_path = directory.path().join("schema.json");
    let output_path = directory.path().join("answer.json");
    fs::write(&schema_path, &schema_text).map_err(|_| failed("Could not prepare the agent run"))?;
    let args = expand_args(run, &schema_text, &schema_path, &output_path);

    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut process = command
        .spawn()
        .map_err(|_| failed("Could not start the agent"))?;
    let mut stdin = process
        .stdin
        .take()
        .ok_or_else(|| failed("Could not start the agent"))?;
    let prompt = prompt.to_owned();
    let writer = thread::spawn(move || stdin.write_all(prompt.as_bytes()));
    let mut stdout = process
        .stdout
        .take()
        .ok_or_else(|| failed("Could not start the agent"))?;
    let reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        stdout
            .by_ref()
            .take(MAX_ANSWER_BYTES + 1)
            .read_to_end(&mut buffer)
            .map(|_| buffer)
    });

    let deadline = Instant::now() + timeout;
    let mut last_tick = Instant::now();
    let status = loop {
        match process.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = process.kill();
                let _ = process.wait();
                return Err(AgentRunError::TimedOut);
            }
            Ok(None) => {
                if last_tick.elapsed() >= Duration::from_secs(1) {
                    last_tick = Instant::now();
                    if tick().is_err() {
                        let _ = process.kill();
                        let _ = process.wait();
                        return Err(failed("The run was stopped while the agent worked"));
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                let _ = process.kill();
                return Err(failed("Lost track of the agent process"));
            }
        }
    };
    let _ = writer.join();
    let stdout = reader
        .join()
        .map_err(|_| failed("Could not read the agent's answer"))?
        .map_err(|_| failed("Could not read the agent's answer"))?;
    if !status.success() {
        return Err(failed("The agent exited with an error"));
    }
    let document = match run.result.source {
        StructuredSource::Stdout => stdout,
        StructuredSource::OutputFile => {
            let file =
                fs::File::open(&output_path).map_err(|_| failed("The agent wrote no answer"))?;
            let mut buffer = Vec::new();
            file.take(MAX_ANSWER_BYTES + 1)
                .read_to_end(&mut buffer)
                .map_err(|_| failed("Could not read the agent's answer"))?;
            buffer
        }
    };
    if document.len() as u64 > MAX_ANSWER_BYTES {
        return Err(failed("The agent's answer is too large"));
    }
    extract_answer(run, &document)
}

fn expand_args(
    run: &StructuredRun,
    schema: &str,
    schema_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Vec<std::ffi::OsString> {
    run.args
        .iter()
        .map(|arg| match arg.as_str() {
            "{schema}" => schema.into(),
            "{schemaPath}" => schema_path.as_os_str().to_owned(),
            "{outputPath}" => output_path.as_os_str().to_owned(),
            other => other.into(),
        })
        .collect()
}

fn extract_answer(run: &StructuredRun, document: &[u8]) -> Result<Value, AgentRunError> {
    let unreadable = AgentRunError::Failed("The agent's answer was not valid JSON");
    let document: Value = serde_json::from_slice(document).map_err(|_| unreadable)?;
    if let Some(pointer) = &run.result.error_pointer
        && document.pointer(pointer).and_then(Value::as_bool) == Some(true)
    {
        return Err(AgentRunError::Failed(
            "The agent reported that it could not finish",
        ));
    }
    let answer = match run.result.pointer.as_deref() {
        None | Some("") => document,
        Some(pointer) => document
            .pointer(pointer)
            .cloned()
            .ok_or(AgentRunError::Failed("The agent's answer was missing"))?,
    };
    // Some CLIs return the structured answer as a JSON string.
    match answer {
        Value::String(text) => serde_json::from_str(&text)
            .map_err(|_| AgentRunError::Failed("The agent's answer was not valid JSON")),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_catalog::StructuredResult;

    fn run(source: StructuredSource, pointer: Option<&str>, error: Option<&str>) -> StructuredRun {
        StructuredRun {
            args: vec!["{schema}".into(), "{outputPath}".into()],
            result: StructuredResult {
                source,
                pointer: pointer.map(Into::into),
                error_pointer: error.map(Into::into),
            },
        }
    }

    #[test]
    fn answers_are_read_from_pointers_and_reported_failures_are_errors() {
        let stdout = run(
            StructuredSource::Stdout,
            Some("/structured_output"),
            Some("/is_error"),
        );
        assert_eq!(
            extract_answer(
                &stdout,
                br#"{"is_error":false,"structured_output":{"summary":"ok"}}"#
            )
            .unwrap(),
            serde_json::json!({"summary": "ok"})
        );
        assert!(extract_answer(&stdout, br#"{"is_error":true,"structured_output":{}}"#).is_err());
        assert!(extract_answer(&stdout, br#"{"is_error":false}"#).is_err());
        let file = run(StructuredSource::OutputFile, None, None);
        assert_eq!(
            extract_answer(&file, br#"{"a":1}"#).unwrap(),
            serde_json::json!({"a": 1})
        );
        assert_eq!(
            extract_answer(&file, br#""{\"a\":1}""#).unwrap(),
            serde_json::json!({"a": 1})
        );
        assert!(extract_answer(&file, b"not json").is_err());
    }

    #[test]
    fn placeholders_expand_only_as_whole_arguments() {
        let declared = StructuredRun {
            args: vec![
                "--schema".into(),
                "{schema}".into(),
                "x{schema}".into(),
                "{schemaPath}".into(),
                "{outputPath}".into(),
            ],
            result: StructuredResult {
                source: StructuredSource::OutputFile,
                pointer: None,
                error_pointer: None,
            },
        };
        let args = expand_args(
            &declared,
            "{}",
            std::path::Path::new("/s.json"),
            std::path::Path::new("/o.json"),
        );
        assert_eq!(
            args,
            ["--schema", "{}", "x{schema}", "/s.json", "/o.json"].map(std::ffi::OsString::from)
        );
    }
}
