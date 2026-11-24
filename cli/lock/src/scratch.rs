use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::Error;

pub struct Scratch {
    root: tempfile::TempDir,
    set: PathBuf,
}

pub const RESOLVER_TIMEOUT: Duration = Duration::from_secs(30 * 60);

const DIRS: [(&str, &[&str]); 4] = [
    ("home", &["HOME", "USERPROFILE"]),
    ("cache", &["XDG_CACHE_HOME", "LOCALAPPDATA"]),
    ("config", &["XDG_CONFIG_HOME", "APPDATA"]),
    ("tmp", &["TMPDIR", "TEMP", "TMP"]),
];

impl Scratch {
    pub fn new() -> Result<Self, Error> {
        let failed = |error: std::io::Error| Error::new(format!("scratch directory: {error}"));
        let root = tempfile::Builder::new()
            .prefix("dx-lock-")
            .tempdir()
            .map_err(failed)?;
        let set = root.path().join("set");
        std::fs::create_dir_all(&set).map_err(failed)?;
        for (dir, _) in DIRS {
            std::fs::create_dir_all(root.path().join(dir)).map_err(failed)?;
        }
        Ok(Self { root, set })
    }

    #[must_use]
    pub fn set(&self) -> &Path {
        &self.set
    }

    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.root.path().join("tmp")
    }

    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command.env_clear().env("PATH", "").current_dir(&self.set);
        for (dir, variables) in DIRS {
            for variable in variables {
                command.env(variable, self.root.path().join(dir));
            }
        }
        command
    }

    pub fn run(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, Error> {
        let captured = dx_process::capture::run_captured(command, timeout)
            .map_err(|error| Error::new(error.to_string()))?;
        if captured.success() {
            return Ok(captured.stdout);
        }
        let status = captured
            .code
            .map_or_else(|| "a signal".to_owned(), |code| format!("exit {code}"));
        Err(Error::new(format!(
            "{} failed with {status}:\n{}{}",
            captured.command,
            captured.stdout_text(),
            captured.stderr_text()
        )))
    }
}
