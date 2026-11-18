//! What the operations share: failures, scratch directories and the process runner.

use std::path::Path;
use std::process::{Command, Stdio};

/// Why an operation stopped, with its exit code.
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

/// The exit code of an operation, or why it stopped.
pub type Outcome = Result<u8, Failure>;

/// Reads one environment variable; empty values count as unset.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// A bad invocation: exit code 2.
pub fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: 2,
        message: message.into(),
    }
}

/// A rejected input: exit code 1.
pub fn reject(message: impl Into<String>) -> Failure {
    Failure {
        code: 1,
        message: message.into(),
    }
}

/// A missing tool: exit code 127.
pub fn missing(message: impl Into<String>) -> Failure {
    Failure {
        code: 127,
        message: message.into(),
    }
}

/// The process environment, with empty values dropped.
#[must_use]
pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Maps a child's exit code to this program's.
#[must_use]
pub fn exit_code(code: Option<i32>) -> u8 {
    code.and_then(|code| u8::try_from(code).ok()).unwrap_or(1)
}

/// One program start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// What a captured run printed. `code` is `None` when the program did not start or was killed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Starts programs; tests replace it.
pub trait Runner {
    /// Runs `cmd` with no input and captures its output.
    fn capture(&self, cmd: &Cmd) -> Output;
    /// Runs `cmd` with this process's streams and returns its exit code.
    fn inherit(&self, cmd: &Cmd) -> Option<i32>;
}

/// Starts real processes.
pub struct System;

fn command(cmd: &Cmd) -> Command {
    let mut command = Command::new(&cmd.program);
    command.args(&cmd.args).envs(cmd.env.iter().cloned());
    command.stdin(Stdio::null());
    command
}

impl Runner for System {
    fn capture(&self, cmd: &Cmd) -> Output {
        match command(cmd).output() {
            Ok(output) => Output {
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
            Err(error) => Output {
                code: None,
                stdout: String::new(),
                stderr: error.to_string(),
            },
        }
    }

    fn inherit(&self, cmd: &Cmd) -> Option<i32> {
        command(cmd).status().ok().and_then(|status| status.code())
    }
}

/// A new directory below `TMPDIR`, removed when the operation ends.
///
/// # Errors
///
/// Returns a usage failure when `TMPDIR` is unset or the directory cannot be made.
pub fn scratch(env: Env<'_>, name: &str) -> Result<tempfile::TempDir, Failure> {
    let root = env("TMPDIR").ok_or_else(|| usage("TMPDIR names the scratch directory"))?;
    tempfile::Builder::new()
        .prefix(&format!("dx-{name}."))
        .tempdir_in(&root)
        .map_err(|error| {
            usage(format!(
                "cannot create a scratch directory in {root}: {error}"
            ))
        })
}

/// Whether `name` ends in `.ext`, ignoring ASCII case.
#[must_use]
pub fn has_extension(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(ext))
}
