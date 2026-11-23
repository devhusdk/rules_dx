use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::lifecycle::{
    CapturePolicy, ChildOutcome, CleanupBudget, Descendants, Exit, run_command_with_input,
};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

const GIT_ENV: [&str; 5] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
];

#[must_use]
pub fn git(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    command
}

#[must_use]
pub fn git_isolated(cwd: &Path) -> Command {
    let mut command = git(cwd);
    for name in GIT_ENV {
        command.env_remove(name);
    }
    command
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunOptions {
    pub foreground: bool,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub command: String,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Captured {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    #[must_use]
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    #[must_use]
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).trim().to_owned()
    }

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
    #[must_use]
    pub fn stderr(&self) -> &str {
        match self {
            RunError::Failed { stderr, .. } => stderr,
            RunError::Spawn { .. }
            | RunError::TimedOut { .. }
            | RunError::OutputTooLarge { .. } => "",
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Spawn { command, source } => write!(f, "cannot run {command}: {source}"),
            RunError::TimedOut { command, timeout } => {
                write!(f, "{command} timed out after {timeout:?}")
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
            RunError::TimedOut { .. }
            | RunError::OutputTooLarge { .. }
            | RunError::Failed { .. } => None,
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

pub fn run_captured(command: &mut Command, timeout: Duration) -> Result<Captured, RunError> {
    run_captured_with_input(command, None, timeout)
}

pub fn run_captured_with_input(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, RunError> {
    run_captured_with(command, input, timeout, RunOptions::default())
}

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

pub fn run_checked(command: &mut Command, timeout: Duration) -> Result<Captured, RunError> {
    run_captured(command, timeout)?.into_success()
}

pub fn run_checked_with(
    command: &mut Command,
    timeout: Duration,
    options: RunOptions,
) -> Result<Captured, RunError> {
    run_captured_with(command, None, timeout, options)?.into_success()
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;
