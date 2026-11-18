//! What the operations share: failures, scratch directories and captured runs.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Why an operation stopped, with its exit code.
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

/// The exit code of an operation, or why it stopped.
pub type Outcome = Result<u8, Failure>;

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

/// A new directory next to `base`, named after it; removed when the operation ends.
///
/// # Errors
///
/// Returns an error when the directory cannot be made.
pub fn scratch_beside(base: &Path) -> std::io::Result<tempfile::TempDir> {
    let mut prefix = base.file_name().unwrap_or_default().to_os_string();
    prefix.push(".dx-work.");
    tempfile::Builder::new()
        .prefix(&prefix)
        .tempdir_in(base.parent().unwrap_or_else(|| Path::new("")))
}

/// Whether `path` is a file this process may run.
#[must_use]
pub fn runnable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// Resolves a path the user typed against the directory `bazel run` started in.
pub fn caller_path(value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = std::env::var_os("BUILD_WORKING_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    base.join(path)
}

/// What one captured run printed and whether it passed.
pub struct Captured {
    pub code: Option<i32>,
    pub text: String,
}

impl Captured {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Whether a line starts with `prefix`.
    #[must_use]
    pub fn line_starts(&self, prefix: &str) -> bool {
        self.text.lines().any(|line| line.starts_with(prefix))
    }

    /// The first line that is not blank and not a stack frame.
    #[must_use]
    pub fn first_reason(&self) -> &str {
        self.text
            .lines()
            .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("at "))
            .unwrap_or_default()
    }
}

/// Runs `command` with no input and returns its output, stdout first.
pub fn capture(command: &mut Command) -> Captured {
    match command.stdin(Stdio::null()).output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            Captured {
                code: output.status.code(),
                text,
            }
        }
        Err(error) => Captured {
            code: None,
            text: error.to_string(),
        },
    }
}

/// Runs `command` with this process's streams and reports whether it passed.
pub fn passes(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The `.apk` files directly inside `dir`, sorted.
#[must_use]
pub fn apks_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|end| end == "apk") && path.is_file())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}
