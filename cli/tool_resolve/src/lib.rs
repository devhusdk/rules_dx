//! Resolves one pinned tool label to the absolute artifact path CI exports.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RESOLVE_CODE: &str = "tool_resolve_failed";

/// Every way resolving one tool label can fail.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("{RESOLVE_CODE}: {detail}")]
    Usage { detail: String },
    #[error("{RESOLVE_CODE}: cquery for {label} failed: {detail}")]
    CqueryFailed { label: String, detail: String },
    #[error("{RESOLVE_CODE}: cquery for {label} produced no artifact path")]
    NoArtifact { label: String },
    #[error("{RESOLVE_CODE}: bazel info {key} failed: {detail}")]
    InfoFailed { key: String, detail: String },
    #[error("{RESOLVE_CODE}: resolved {path} for {label}, but no file is there")]
    MissingArtifact { label: String, path: String },
    #[error(
        "{RESOLVE_CODE}: resolved {path} for {label}, but no file is there; build {label} first"
    )]
    MissingOutput { label: String, path: String },
    #[error("{RESOLVE_CODE}: could not append {var} to {path}: {detail}")]
    EnvWrite {
        var: String,
        path: String,
        detail: String,
    },
}

impl From<ResolveError> for dx_error::DxError {
    fn from(error: ResolveError) -> Self {
        let message = error.to_string();
        match error {
            ResolveError::Usage { .. } => Self::usage(message),
            _ => Self::infra(message),
        }
    }
}

/// How one resolution reaches the Bazel server.
pub trait Bazel {
    fn cquery_files(&self, label: &str, args: &[String]) -> Result<String, ResolveError>;
    fn output_base(&self) -> Result<String, ResolveError>;
    fn execution_root(&self) -> Result<String, ResolveError>;
}

/// The real Bazel child processes.
pub struct CommandBazel {
    pub bin: PathBuf,
    pub dir: PathBuf,
}

const BAZEL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1800);

fn run_detail(bin: &Path, error: &dx_process::capture::RunError) -> String {
    match error {
        dx_process::capture::RunError::Spawn { source, .. } => {
            format!("could not run {}: {source}", bin.display())
        }
        other => other.to_string(),
    }
}

impl Bazel for CommandBazel {
    fn cquery_files(&self, label: &str, args: &[String]) -> Result<String, ResolveError> {
        let mut argv = vec![
            "cquery".to_owned(),
            "--output=files".to_owned(),
            label.to_owned(),
        ];
        argv.extend(args.iter().cloned());
        let output = dx_process::capture::run_captured_with(
            Command::new(&self.bin).args(&argv).current_dir(&self.dir),
            None,
            BAZEL_TIMEOUT,
            dx_process::capture::RunOptions {
                foreground: true,
                ..Default::default()
            },
        )
        .map_err(|error| ResolveError::CqueryFailed {
            label: label.to_owned(),
            detail: run_detail(&self.bin, &error),
        })?;
        if !output.success() {
            return Err(ResolveError::CqueryFailed {
                label: label.to_owned(),
                detail: exit_detail(output.code, &output.stderr),
            });
        }
        Ok(output.stdout_text())
    }

    fn output_base(&self) -> Result<String, ResolveError> {
        self.info("output_base")
    }

    fn execution_root(&self) -> Result<String, ResolveError> {
        self.info("execution_root")
    }
}

impl CommandBazel {
    fn info(&self, key: &str) -> Result<String, ResolveError> {
        let failed = |detail: String| ResolveError::InfoFailed {
            key: key.to_owned(),
            detail,
        };
        let output = dx_process::capture::run_captured_with(
            Command::new(&self.bin)
                .args(["info", key])
                .current_dir(&self.dir),
            None,
            BAZEL_TIMEOUT,
            dx_process::capture::RunOptions {
                foreground: true,
                ..Default::default()
            },
        )
        .map_err(|error| failed(run_detail(&self.bin, &error)))?;
        if !output.success() {
            return Err(failed(exit_detail(output.code, &output.stderr)));
        }
        let value = output.stdout_text().trim().to_owned();
        if value.is_empty() {
            return Err(failed(format!("bazel info {key} printed nothing")));
        }
        Ok(value)
    }
}

/// Returns the directory inner Bazel commands run in.
///
/// An explicit workspace wins. Otherwise `bazel run` publishes its invocation
/// root as `BUILD_WORKSPACE_DIRECTORY`, which matters because the binary
/// itself executes below `bazel-bin` and Bazel refuses output directories.
/// The current directory is the last resort.
pub fn context_dir(
    explicit: Option<&Path>,
    build_workspace_dir: Option<&std::ffi::OsStr>,
    current: &Path,
) -> PathBuf {
    match explicit {
        Some(dir) => dir.to_owned(),
        None => build_workspace_dir
            .map(PathBuf::from)
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or_else(|| current.to_owned()),
    }
}

/// Returns the first non-empty stderr line, the one-line failure summary.
pub fn first_line(stderr: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

/// Renders one inner Bazel failure as `exit <code>[: <first stderr line>]`.
pub fn exit_detail(code: Option<i32>, stderr: &[u8]) -> String {
    let code = code.unwrap_or(-1);
    match first_line(stderr) {
        Some(line) => format!("exit {code}: {line}"),
        None => format!("exit {code}"),
    }
}

/// Returns whether `name` is a plain environment variable name.
pub fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

/// Validates the exported variable name.
pub fn validate_var(var: &str) -> Result<(), ResolveError> {
    if is_var_name(var) {
        Ok(())
    } else {
        Err(ResolveError::Usage {
            detail: format!("--var {var:?} is not a plain environment variable name"),
        })
    }
}

/// Validates the tool label before Bazel ever runs.
pub fn validate_label(label: &str) -> Result<(), ResolveError> {
    if label.is_empty() || label.chars().any(char::is_whitespace) {
        return Err(ResolveError::Usage {
            detail: format!("--label {label:?} is not a plain Bazel label"),
        });
    }
    Ok(())
}

/// Returns the last non-empty cquery output line, the artifact path.
pub fn select_artifact(cquery_stdout: &str, label: &str) -> Result<String, ResolveError> {
    cquery_stdout
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| ResolveError::NoArtifact {
            label: label.to_owned(),
        })
}

/// Joins one root directory with one cquery artifact path.
pub fn join_under(root: &str, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(PathBuf::from(root), |path, part| path.join(part))
}

/// Renders one `NAME=value` environment file line.
pub fn env_line(var: &str, path: &Path) -> Result<String, ResolveError> {
    validate_var(var)?;
    let rendered = path.display().to_string();
    if rendered.contains('\n') || rendered.contains('\r') {
        return Err(ResolveError::Usage {
            detail: format!("resolved path {rendered:?} cannot cross lines"),
        });
    }
    Ok(format!("{var}={rendered}\n"))
}

/// Appends one line to the environment file, creating it when missing.
pub fn append_env_file(env_file: &Path, line: &str) -> io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut handle = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env_file)?;
    handle.write_all(line.as_bytes())
}

/// Resolves one tool label to its absolute artifact path, failing closed.
pub fn resolve(
    bazel: &dyn Bazel,
    label: &str,
    cquery_args: &[String],
) -> Result<PathBuf, ResolveError> {
    validate_label(label)?;
    let stdout = bazel.cquery_files(label, cquery_args)?;
    let rel = select_artifact(&stdout, label)?;
    let external = rel.starts_with("external/");
    let root = if external {
        bazel.output_base()?
    } else {
        bazel.execution_root()?
    };
    let path = join_under(&root, &rel);
    if !path.is_file() {
        let label = label.to_owned();
        let path = path.display().to_string();
        return Err(if external {
            ResolveError::MissingArtifact { label, path }
        } else {
            ResolveError::MissingOutput { label, path }
        });
    }
    Ok(path)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
