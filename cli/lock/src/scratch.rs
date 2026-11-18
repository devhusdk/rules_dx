//! The scratch directory a resolver runs in.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::Error;

/// A directory that holds a copy of the set and the resolver's home, cache and config; removed on drop.
pub struct Scratch {
    root: tempfile::TempDir,
    set: PathBuf,
}

/// How long a resolver's package manager may run.
pub const RESOLVER_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The scratch directories and the variables that point a program at each.
const DIRS: [(&str, &[&str]); 4] = [
    ("home", &["HOME", "USERPROFILE"]),
    ("cache", &["XDG_CACHE_HOME", "LOCALAPPDATA"]),
    ("config", &["XDG_CONFIG_HOME", "APPDATA"]),
    ("tmp", &["TMPDIR", "TEMP", "TMP"]),
];

impl Scratch {
    /// Makes the scratch directories.
    ///
    /// # Errors
    /// Fails when a directory cannot be made.
    pub fn new() -> Result<Self, Error> {
        let failed = |error: std::io::Error| Error::new(format!("scratch directory: {error}"));
        let root = tempfile::Builder::new()
            .prefix("dx-lock-")
            .tempdir()
            .map_err(failed)?;
        let set = root.path().join("set");
        std::fs::create_dir(&set).map_err(failed)?;
        for (dir, _) in DIRS {
            std::fs::create_dir(root.path().join(dir)).map_err(failed)?;
        }
        Ok(Self { root, set })
    }

    /// The directory that holds the manifest and state files.
    #[must_use]
    pub fn set(&self) -> &Path {
        &self.set
    }

    /// The scratch temporary directory.
    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.root.path().join("tmp")
    }

    /// The scratch home directory.
    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    /// A command that runs `program` in the set directory with an empty `PATH` and no host environment.
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

    /// Runs `command`, killing it and its descendants after `timeout`, and returns its stdout.
    ///
    /// # Errors
    /// Fails when the command cannot start, times out or exits non-zero; the message carries its output.
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
