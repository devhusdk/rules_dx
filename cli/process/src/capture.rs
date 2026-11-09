//! Bounded subprocess capture and the shared git launcher.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::lifecycle::{
    CapturePolicy, ChildOutcome, CleanupBudget, Descendants, Exit, run_command_with_input,
};

/// Bound applied by callers that have no better figure for a short query.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

const GIT_ENV: [&str; 5] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
];

/// A git command in `cwd`, inheriting the environment.
pub fn git(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    command
}

/// A git command in `cwd` without the inherited repository selection.
pub fn git_isolated(cwd: &Path) -> Command {
    let mut command = git(cwd);
    for name in GIT_ENV {
        command.env_remove(name);
    }
    command
}

/// How a captured run behaves beyond its timeout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunOptions {
    /// Keep the child in dx's process group so terminal signals and prompts reach it.
    pub foreground: bool,
    /// Fail with `RunError::OutputTooLarge` once a stream exceeds this many bytes.
    pub max_bytes: Option<usize>,
}

/// What a finished child wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    /// The command line, for messages.
    pub command: String,
    /// The exit code, or `None` when a signal ended the child.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Captured {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).trim().to_owned()
    }

    /// Turns a non-zero exit into `RunError::Failed`.
    pub fn into_success(self) -> Result<Self, RunError> {
        if self.success() {
            return Ok(self);
        }
        Err(RunError::Failed {
            stderr: self.stderr_text(),
            command: self.command,
            code: self.code,
        })
    }
}

/// Why a captured run produced no usable result.
#[derive(Debug)]
pub enum RunError {
    Spawn {
        command: String,
        source: io::Error,
    },
    TimedOut {
        command: String,
        timeout: Duration,
    },
    OutputTooLarge {
        command: String,
        limit: usize,
    },
    Failed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl RunError {
    /// The captured stderr of a failed run, empty for the other kinds.
    pub fn stderr(&self) -> &str {
        match self {
            RunError::Failed { stderr, .. } => stderr,
            _ => "",
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Spawn { command, source } => write!(f, "cannot run {command}: {source}"),
            RunError::TimedOut { command, timeout } => {
                write!(f, "{command} timed out after {}s", timeout.as_secs())
            }
            RunError::OutputTooLarge { command, limit } => {
                write!(f, "{command} wrote more than {limit} bytes")
            }
            RunError::Failed {
                command,
                code,
                stderr,
            } => {
                let status = match code {
                    Some(code) => format!("exit {code}"),
                    None => "a signal".to_owned(),
                };
                if stderr.is_empty() {
                    write!(f, "{command} failed with {status}")
                } else {
                    write!(f, "{command} failed: {stderr}")
                }
            }
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RunError::Spawn { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn describe(command: &Command) -> String {
    let program = Path::new(command.get_program())
        .file_name()
        .map_or_else(|| command.get_program(), OsStr::new);
    std::iter::once(program)
        .chain(command.get_args())
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs `command` with stdin null and both output streams captured.
///
/// The child and its descendants stop once `timeout` passes. A non-zero exit
/// is a result; use `Captured::into_success` or `run_checked` to reject it.
pub fn run_captured(command: &mut Command, timeout: Duration) -> Result<Captured, RunError> {
    run_captured_with_input(command, None, timeout)
}

/// Like `run_captured`, and writes `input` to the child's stdin.
pub fn run_captured_with_input(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, RunError> {
    run_captured_with(command, input, timeout, RunOptions::default())
}

/// Like `run_captured_with_input`, with `options`.
pub fn run_captured_with(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
    options: RunOptions,
) -> Result<Captured, RunError> {
    let label = describe(command);
    let capture = CapturePolicy {
        max_bytes: options.max_bytes.unwrap_or(usize::MAX),
    };
    let outcome = run_command_with_input(
        command,
        input,
        &capture,
        timeout,
        if options.foreground {
            Descendants::Foreground
        } else {
            Descendants::Owned
        },
        CleanupBudget::STANDARD,
        &|| None,
    )
    .map_err(|source| RunError::Spawn {
        command: label.clone(),
        source,
    })?;
    match outcome {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => Ok(Captured {
            command: label,
            code: match exit {
                Exit::Code(code) => Some(code),
                Exit::Signal(_) => None,
            },
            stdout,
            stderr,
        }),
        ChildOutcome::TimedOut => Err(RunError::TimedOut {
            command: label,
            timeout,
        }),
        ChildOutcome::OutputTooLarge { limit } => Err(RunError::OutputTooLarge {
            command: label,
            limit,
        }),
    }
}

/// Like `run_captured`, and a non-zero exit is `RunError::Failed`.
pub fn run_checked(command: &mut Command, timeout: Duration) -> Result<Captured, RunError> {
    run_captured(command, timeout)?.into_success()
}

/// Like `run_checked`, with `options`.
pub fn run_checked_with(
    command: &mut Command,
    timeout: Duration,
    options: RunOptions,
) -> Result<Captured, RunError> {
    run_captured_with(command, None, timeout, options)?.into_success()
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
#[path = "capture_tests.rs"]
mod capture_tests;
