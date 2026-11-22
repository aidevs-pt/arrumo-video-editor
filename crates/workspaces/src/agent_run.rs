//! Runs an agent provider's declared structured run: one non-interactive CLI call that answers a
//! prompt with JSON following a schema. Provider flags come from the plugin manifest.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
    sync::Mutex,
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
/// How much of each output stream a saved log keeps.
pub(crate) const MAX_LOG_BYTES: usize = 256 * 1024;

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

/// What one agent run was asked and printed, readable while it is still running.
#[derive(Default)]
pub(crate) struct AgentTranscript {
    pub command: Mutex<String>,
    pub output: Mutex<Vec<u8>>,
    pub errors: Mutex<Vec<u8>>,
    pub exit_code: Mutex<Option<i32>>,
}

impl AgentTranscript {
    pub(crate) fn output_text(&self) -> String {
        tail_text(
            &self
                .output
                .lock()
                .map(|bytes| bytes.clone())
                .unwrap_or_default(),
        )
    }

    pub(crate) fn errors_text(&self) -> String {
        tail_text(
            &self
                .errors
                .lock()
                .map(|bytes| bytes.clone())
                .unwrap_or_default(),
        )
    }
}

/// Keeps the end of long output, where failures are reported.
fn tail_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= MAX_LOG_BYTES {
        return text.into_owned();
    }
    let mut start = text.len() - MAX_LOG_BYTES;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!("…\n{}", &text[start..])
}

fn capture(mut stream: impl Read, sink: &Mutex<Vec<u8>>, limit: u64) -> io::Result<()> {
    let mut buffer = [0; 8192];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        let mut sink = sink.lock().map_err(|_| io::Error::other("poisoned"))?;
        if (sink.len() as u64) < limit {
            sink.extend_from_slice(&buffer[..read]);
        }
    }
}

/// Runs the agent once, recording what it prints in `transcript`. `tick` is called about every
/// second while it works, so callers can renew leases, save progress, or stop early by returning
/// an error.
pub(crate) fn run_structured(
    plugin: &AgentPlugin,
    prompt: &str,
    schema: &Value,
    timeout: Duration,
    settings: &BTreeMap<String, Value>,
    images: &[std::path::PathBuf],
    transcript: &AgentTranscript,
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
    let args = expand_args(
        run,
        settings,
        images,
        &schema_text,
        &schema_path,
        &output_path,
    );
    if let Ok(mut command) = transcript.command.lock() {
        // Image paths are local; the log only says how many were attached.
        *command = describe_command(&executable, &declared_args(run, settings, &[]));
        if !images.is_empty() {
            command.push_str(&format!(" (+{} images)", images.len().min(MAX_IMAGES)));
        }
    }

    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| failed("Could not start the agent"))?;
    let stderr = process
        .stderr
        .take()
        .ok_or_else(|| failed("Could not start the agent"))?;
    let prompt = prompt.to_owned();

    let status = thread::scope(|scope| {
        scope.spawn(move || stdin.write_all(prompt.as_bytes()));
        let reader = scope.spawn(|| capture(stdout, &transcript.output, MAX_ANSWER_BYTES + 1));
        let errors = scope.spawn(|| capture(stderr, &transcript.errors, MAX_ANSWER_BYTES));
        let deadline = Instant::now() + timeout;
        let mut last_tick = Instant::now();
        let status = loop {
            match process.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = process.kill();
                    let _ = process.wait();
                    break Err(AgentRunError::TimedOut);
                }
                Ok(None) => {
                    if last_tick.elapsed() >= Duration::from_secs(1) {
                        last_tick = Instant::now();
                        if tick().is_err() {
                            let _ = process.kill();
                            let _ = process.wait();
                            break Err(failed("The run was stopped while the agent worked"));
                        }
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => {
                    let _ = process.kill();
                    let _ = process.wait();
                    break Err(failed("Lost track of the agent process"));
                }
            }
        };
        let _ = errors.join();
        if reader.join().map_or(true, |read| read.is_err()) && status.is_ok() {
            return Err(failed("Could not read the agent's answer"));
        }
        status
    })?;
    if let Ok(mut code) = transcript.exit_code.lock() {
        *code = status.code();
    }
    if !status.success() {
        return Err(failed("The agent exited with an error"));
    }
    let document = match run.result.source {
        StructuredSource::Stdout => transcript
            .output
            .lock()
            .map(|bytes| bytes.clone())
            .map_err(|_| failed("Could not read the agent's answer"))?,
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

/// The command line as the plugin declares it, with the schema left as a placeholder.
fn describe_command(executable: &std::path::Path, args: &[String]) -> String {
    let name = executable
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    std::iter::once(name)
        .chain(args.iter().map(|arg| {
            if arg.is_empty() || arg.contains(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            {
                format!("'{}'", arg.replace('\'', "'\\''"))
            } else {
                arg.clone()
            }
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The plugin's declared arguments for the given setting values. Values are used only when they
/// are plain words (and, for settings with choices, one of them), so they cannot add arguments.
pub(crate) fn setting_arguments(
    run: &StructuredRun,
    settings: &BTreeMap<String, Value>,
) -> Vec<String> {
    let plain = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
            })
    };
    let mut arguments = Vec::new();
    for entry in &run.setting_args {
        match settings.get(&entry.setting) {
            Some(Value::Bool(true)) => arguments.extend(entry.args.iter().cloned()),
            Some(Value::Bool(false)) | None | Some(Value::Null) => {
                arguments.extend(entry.otherwise.iter().flatten().cloned())
            }
            Some(value) => {
                let text = match value {
                    Value::String(text) => text.trim().to_owned(),
                    Value::Number(number) if number.is_i64() => number.to_string(),
                    _ => String::new(),
                };
                if plain(&text) {
                    arguments.extend(entry.args.iter().map(|arg| arg.replace("{value}", &text)));
                } else {
                    arguments.extend(entry.otherwise.iter().flatten().cloned());
                }
            }
        }
    }
    arguments
}

/// The declared arguments with `{settings}` and `{images}` expanded; file placeholders stay as
/// written.
fn declared_args(
    run: &StructuredRun,
    settings: &BTreeMap<String, Value>,
    images: &[std::path::PathBuf],
) -> Vec<String> {
    run.args
        .iter()
        .flat_map(|arg| match arg.as_str() {
            "{settings}" => setting_arguments(run, settings),
            "{images}" => image_arguments(run, images),
            _ => vec![arg.clone()],
        })
        .collect()
}

/// The plugin's image arguments repeated for each image; none when it cannot take images.
fn image_arguments(run: &StructuredRun, images: &[std::path::PathBuf]) -> Vec<String> {
    let Some(template) = &run.image_args else {
        return Vec::new();
    };
    images
        .iter()
        .take(MAX_IMAGES)
        .flat_map(|path| {
            let path = path.to_string_lossy();
            template
                .iter()
                .map(move |arg| arg.replace("{imagePath}", &path))
        })
        .collect()
}

/// Whether the agent can look at images it is given.
pub(crate) fn accepts_images(plugin: &AgentPlugin) -> bool {
    plugin
        .cli
        .structured_run
        .as_ref()
        .is_some_and(|run| run.image_args.is_some() && run.args.iter().any(|arg| arg == "{images}"))
}

/// The most images one agent run is shown.
pub(crate) const MAX_IMAGES: usize = 8;

fn expand_args(
    run: &StructuredRun,
    settings: &BTreeMap<String, Value>,
    images: &[std::path::PathBuf],
    schema: &str,
    schema_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Vec<std::ffi::OsString> {
    declared_args(run, settings, images)
        .into_iter()
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
            setting_args: Vec::new(),
            image_args: None,
            progress: Vec::new(),
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
            setting_args: Vec::new(),
            image_args: None,
            progress: Vec::new(),
            result: StructuredResult {
                source: StructuredSource::OutputFile,
                pointer: None,
                error_pointer: None,
            },
        };
        let args = expand_args(
            &declared,
            &BTreeMap::new(),
            &[],
            "{}",
            std::path::Path::new("/s.json"),
            std::path::Path::new("/o.json"),
        );
        assert_eq!(
            args,
            ["--schema", "{}", "x{schema}", "/s.json", "/o.json"].map(std::ffi::OsString::from)
        );
    }

    #[test]
    fn settings_choose_declared_arguments_and_reject_unsafe_values() {
        use crate::plugin_catalog::SettingArgs;
        let declared = StructuredRun {
            args: vec!["exec".into(), "{settings}".into(), "-".into()],
            setting_args: vec![
                SettingArgs {
                    setting: "web-search".into(),
                    args: vec!["-c".into(), "web_search=\"live\"".into()],
                    otherwise: Some(vec!["-c".into(), "web_search=\"disabled\"".into()]),
                },
                SettingArgs {
                    setting: "model".into(),
                    args: vec!["-m".into(), "{value}".into()],
                    otherwise: None,
                },
            ],
            image_args: None,
            progress: Vec::new(),
            result: StructuredResult {
                source: StructuredSource::Stdout,
                pointer: None,
                error_pointer: None,
            },
        };
        let settings = BTreeMap::from([
            ("web-search".to_owned(), serde_json::json!(true)),
            ("model".to_owned(), serde_json::json!("gpt-5.1")),
        ]);
        assert_eq!(
            declared_args(&declared, &settings, &[]),
            ["exec", "-c", "web_search=\"live\"", "-m", "gpt-5.1", "-"]
        );
        let unsafe_model = BTreeMap::from([("model".to_owned(), serde_json::json!("x --yolo"))]);
        assert_eq!(
            declared_args(&declared, &unsafe_model, &[]),
            ["exec", "-c", "web_search=\"disabled\"", "-"]
        );
    }
}
