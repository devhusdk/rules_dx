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

/// A directory removed when the operation ends.
pub struct Scratch(PathBuf);

impl Scratch {
    /// Creates a new directory named after `base` and this process.
    pub fn beside(base: &Path) -> std::io::Result<Scratch> {
        let mut name = base.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".dx-work.{}", std::process::id()));
        let path = base.with_file_name(name);
        std::fs::create_dir_all(&path)?;
        Ok(Scratch(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether `path` is a file this process may run.
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
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Whether a line starts with `prefix`.
    pub fn line_starts(&self, prefix: &str) -> bool {
        self.text.lines().any(|line| line.starts_with(prefix))
    }

    /// The first line that is not blank and not a stack frame.
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

/// The entry names in the central directory of one zip file.
pub fn zip_names(bytes: &[u8]) -> Option<Vec<String>> {
    let word = |at: usize, len: usize| -> Option<usize> {
        let field = bytes.get(at..at.checked_add(len)?)?;
        Some(
            field
                .iter()
                .rev()
                .fold(0usize, |value, byte| (value << 8) | usize::from(*byte)),
        )
    };
    let end = (0..=bytes.len().checked_sub(22)?)
        .rev()
        .take(65_536)
        .find(|&at| bytes.get(at..at + 4) == Some(b"PK\x05\x06"))?;
    let count = word(end + 10, 2)?;
    let mut at = word(end + 16, 4)?;
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        if bytes.get(at..at.checked_add(4)?) != Some(b"PK\x01\x02") {
            return None;
        }
        let name = word(at + 28, 2)?;
        let extra = word(at + 30, 2)?;
        let comment = word(at + 32, 2)?;
        let text = bytes.get(at + 46..(at + 46).checked_add(name)?)?;
        names.push(String::from_utf8_lossy(text).into_owned());
        at = at + 46 + name + extra + comment;
    }
    Some(names)
}
