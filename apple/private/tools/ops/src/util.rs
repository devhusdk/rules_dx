use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

pub type Outcome = Result<u8, Failure>;

pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

pub fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: 2,
        message: message.into(),
    }
}

pub fn reject(message: impl Into<String>) -> Failure {
    Failure {
        code: 1,
        message: message.into(),
    }
}

pub fn missing(message: impl Into<String>) -> Failure {
    Failure {
        code: 127,
        message: message.into(),
    }
}

#[must_use]
pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[must_use]
pub fn exit_code(code: Option<i32>) -> u8 {
    code.and_then(|code| u8::try_from(code).ok()).unwrap_or(1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

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

pub trait Runner {
    fn capture(&self, cmd: &Cmd) -> Output;
    fn inherit(&self, cmd: &Cmd) -> Option<i32>;
}

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

/// # Errors
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

pub const BUNDLE_LIMITS: dx_archive::Limits = dx_archive::Limits {
    max_entries: 262_144,
    max_entry_bytes: 4 << 30,
    max_total_bytes: 16 << 30,
};

/// # Errors
/// Returns the error of a member that is unsafe, over [`BUNDLE_LIMITS`] or unreadable.
pub fn unzip(archive: &Path, dest: &Path) -> io::Result<()> {
    dx_archive::extract(archive, dx_archive::Format::Zip, dest, BUNDLE_LIMITS).map(drop)
}

#[must_use]
pub fn has_extension(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(ext))
}
