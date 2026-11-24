use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

pub type Outcome = Result<u8, Failure>;

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

/// # Errors
/// Returns an error when the directory cannot be made.
pub fn scratch_beside(base: &Path) -> std::io::Result<tempfile::TempDir> {
    let mut prefix = base.file_name().unwrap_or_default().to_os_string();
    prefix.push(".dx-work.");
    tempfile::Builder::new()
        .prefix(&prefix)
        .tempdir_in(base.parent().unwrap_or_else(|| Path::new("")))
}

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

pub struct Captured {
    pub code: Option<i32>,
    pub text: String,
}

impl Captured {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    #[must_use]
    pub fn line_starts(&self, prefix: &str) -> bool {
        self.text.lines().any(|line| line.starts_with(prefix))
    }

    #[must_use]
    pub fn first_reason(&self) -> &str {
        self.text
            .lines()
            .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("at "))
            .unwrap_or_default()
    }
}

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

pub fn passes(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

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
