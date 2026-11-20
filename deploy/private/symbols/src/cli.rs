//! Flag parsing, tool runs and manifest writes shared by the split, dsym and pdb helpers.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::{Captured, RunError};

use crate::SymbolsError;

/// Why a symbols helper stopped.
#[derive(Debug)]
pub enum CliError {
    UnknownFlag(String),
    MissingValue(String),
    MissingFlag(&'static str),
    NoInput(PathBuf),
    NotUtf8(PathBuf),
    Run { tool: PathBuf, source: RunError },
    ToolFailed { tool: PathBuf, stderr: String },
    Encode(serde_json::Error),
    Symbols(SymbolsError),
    Io(PathIoError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFlag(flag) => write!(f, "unknown flag {flag}"),
            Self::MissingValue(flag) => write!(f, "{flag} needs a value"),
            Self::MissingFlag(flag) => write!(f, "{flag} is required"),
            Self::NoInput(path) => write!(f, "{}: no such file", path.display()),
            Self::NotUtf8(path) => write!(f, "{} is not UTF-8", path.display()),
            Self::Run { tool, source } => write!(f, "cannot run {}: {source}", tool.display()),
            Self::ToolFailed { tool, stderr } => {
                write!(f, "{} failed: {stderr}", tool.display())
            }
            Self::Encode(error) => write!(f, "cannot encode manifest: {error}"),
            Self::Symbols(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Run { source, .. } => Some(source),
            Self::Encode(error) => Some(error),
            Self::Symbols(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::UnknownFlag(_)
            | Self::MissingValue(_)
            | Self::MissingFlag(_)
            | Self::NoInput(_)
            | Self::NotUtf8(_)
            | Self::ToolFailed { .. } => None,
        }
    }
}

impl From<SymbolsError> for CliError {
    fn from(error: SymbolsError) -> Self {
        Self::Symbols(error)
    }
}

impl From<PathIoError> for CliError {
    fn from(error: PathIoError) -> Self {
        Self::Io(error)
    }
}

/// Reads `--flag value` pairs; every name in `names` is required, and the paths come back in that order.
///
/// # Errors
///
/// Returns an error for an unknown flag, a flag without a value, or a missing flag.
pub fn parse_flags<const N: usize>(
    args: &[String],
    names: [&'static str; N],
) -> Result<[PathBuf; N], CliError> {
    let mut found: [Option<PathBuf>; N] = std::array::from_fn(|_| None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let slot = names
            .iter()
            .position(|name| name == flag)
            .and_then(|at| found.get_mut(at))
            .ok_or_else(|| CliError::UnknownFlag(flag.clone()))?;
        let value = it
            .next()
            .ok_or_else(|| CliError::MissingValue(flag.clone()))?;
        *slot = Some(PathBuf::from(value));
    }
    if let Some((name, _)) = names.iter().zip(&found).find(|(_, value)| value.is_none()) {
        return Err(CliError::MissingFlag(name));
    }
    Ok(found.map(Option::unwrap_or_default))
}

/// Runs a tool and returns its output when it exits 0.
///
/// # Errors
///
/// Returns an error when the tool cannot start, times out or exits non-zero.
pub fn run_tool(
    tool: &Path,
    args: &[&std::ffi::OsStr],
    timeout: Duration,
) -> Result<Captured, CliError> {
    let mut command = Command::new(tool);
    command.args(args);
    let out = dx_process::capture::run_captured(&mut command, timeout).map_err(|source| {
        CliError::Run {
            tool: tool.to_path_buf(),
            source,
        }
    })?;
    if !out.success() {
        return Err(CliError::ToolFailed {
            tool: tool.to_path_buf(),
            stderr: out.stderr_text(),
        });
    }
    Ok(out)
}

/// Writes a manifest as pretty JSON with a trailing newline.
///
/// # Errors
///
/// Returns an error when the manifest cannot be encoded or written.
pub fn write_manifest(path: &Path, manifest: &serde_json::Value) -> Result<(), CliError> {
    let mut text = serde_json::to_string_pretty(manifest).map_err(CliError::Encode)?;
    text.push('\n');
    path_io::write_scratch(path, text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn flags_come_back_in_name_order() {
        let [a, b] = parse_flags(&args(&["--b", "2", "--a", "1"]), ["--a", "--b"]).expect("flags");
        assert_eq!((a, b), (PathBuf::from("1"), PathBuf::from("2")));
    }

    #[test]
    fn bad_flags_are_typed_errors() {
        let names = ["--a", "--b"];
        let error = parse_flags(&args(&["--c", "1"]), names).expect_err("unknown");
        assert!(matches!(&error, CliError::UnknownFlag(flag) if flag == "--c"));
        let error = parse_flags(&args(&["--a"]), names).expect_err("no value");
        assert!(matches!(&error, CliError::MissingValue(flag) if flag == "--a"));
        let error = parse_flags(&args(&["--a", "1"]), names).expect_err("missing");
        assert!(matches!(error, CliError::MissingFlag("--b")));
        assert_eq!(error.to_string(), "--b is required");
    }
}
