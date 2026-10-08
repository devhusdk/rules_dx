#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod lifecycle;

pub trait Fs {
    fn is_file(&self, path: &Path) -> bool;
    fn broken_marker_hint(&self, dir: &Path) -> Option<String> {
        let _ = dir;
        None
    }
}

pub struct RealFs;

impl Fs for RealFs {
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn broken_marker_hint(&self, dir: &Path) -> Option<String> {
        let marker = dir.join("MODULE.bazel");
        if self.is_file(&marker) {
            return None;
        }
        if std::fs::symlink_metadata(&marker).is_err() {
            return None;
        }
        match std::fs::metadata(&marker) {
            Ok(_) => Some("symlink target is not a regular file".to_owned()),
            Err(err) => Some(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiscoverError {
    #[error(
        "workspace_not_found: no MODULE.bazel in {searched_len} directorie(s); pass --workspace <path>",
        searched_len = searched.len()
    )]
    NotFound { searched: Vec<PathBuf> },
    #[error(
        "unsupported_workspace: {dir} has WORKSPACE without MODULE.bazel; migrate to Bzlmod",
        dir = dir.display()
    )]
    UnsupportedLegacy { dir: PathBuf },
    #[error("workspace_not_found: --workspace {path} has no MODULE.bazel", path = path.display())]
    InvalidOverride { path: PathBuf },
    #[error("workspace_unreadable: --workspace {path} MODULE.bazel is unreadable ({reason}); check the symlink target or permissions", path = path.display())]
    UnreadableOverride { path: PathBuf, reason: String },
    #[error("workspace_unreadable: {path}/MODULE.bazel is unreadable ({reason}); check the symlink target or permissions", path = path.display())]
    UnreadableMarker { path: PathBuf, reason: String },
}

fn has_module(fs: &dyn Fs, dir: &Path) -> bool {
    fs.is_file(&dir.join("MODULE.bazel"))
}

fn legacy_marker(fs: &dyn Fs, dir: &Path) -> bool {
    fs.is_file(&dir.join("WORKSPACE")) || fs.is_file(&dir.join("WORKSPACE.bazel"))
}

pub fn expand_tilde(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    let is_bare = text == "~";
    let is_home_prefixed = text.starts_with("~/") || text.starts_with("~\\");
    if !is_bare && !is_home_prefixed {
        return path.to_path_buf();
    }
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return path.to_path_buf();
    };
    if is_bare {
        return PathBuf::from(home);
    }
    PathBuf::from(home).join(&text[2..])
}

pub fn canonicalize_or_keep(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn resolve_override_display(raw: &Path, base: &Path) -> PathBuf {
    let expanded = expand_tilde(raw);
    if expanded.is_absolute() {
        expanded
    } else {
        base.join(expanded)
    }
}

pub fn resolve_override_effective(raw: &Path, base: &Path) -> (PathBuf, PathBuf) {
    let display = resolve_override_display(raw, base);
    let effective = canonicalize_or_keep(&display);
    (effective, display)
}

pub fn discover(
    start: &Path,
    override_dir: Option<&Path>,
    fs: &dyn Fs,
) -> Result<PathBuf, DiscoverError> {
    if let Some(dir) = override_dir {
        if has_module(fs, dir) {
            return Ok(dir.to_path_buf());
        }
        if let Some(reason) = fs.broken_marker_hint(dir) {
            return Err(DiscoverError::UnreadableOverride {
                path: dir.to_path_buf(),
                reason,
            });
        }
        if legacy_marker(fs, dir) {
            return Err(DiscoverError::UnsupportedLegacy {
                dir: dir.to_path_buf(),
            });
        }
        return Err(DiscoverError::InvalidOverride {
            path: dir.to_path_buf(),
        });
    }
    let mut searched = Vec::new();
    let mut legacy: Option<PathBuf> = None;
    for dir in start.ancestors() {
        searched.push(dir.to_path_buf());
        if has_module(fs, dir) {
            return Ok(dir.to_path_buf());
        }
        if let Some(reason) = fs.broken_marker_hint(dir) {
            return Err(DiscoverError::UnreadableMarker {
                path: dir.to_path_buf(),
                reason,
            });
        }
        if legacy.is_none() && legacy_marker(fs, dir) {
            legacy = Some(dir.to_path_buf());
        }
    }
    if let Some(dir) = legacy {
        return Err(DiscoverError::UnsupportedLegacy { dir });
    }
    Err(DiscoverError::NotFound { searched })
}

pub fn discover_real(start: &Path, override_dir: Option<&Path>) -> Result<PathBuf, DiscoverError> {
    let effective_start = canonicalize_or_keep(start);
    if let Some(raw) = override_dir {
        let (effective, display) = resolve_override_effective(raw, &effective_start);
        match discover(&effective_start, Some(&effective), &RealFs) {
            Ok(workspace) => Ok(workspace),
            Err(DiscoverError::InvalidOverride { .. }) => {
                Err(DiscoverError::InvalidOverride { path: display })
            }
            Err(DiscoverError::UnsupportedLegacy { .. }) => {
                Err(DiscoverError::UnsupportedLegacy { dir: display })
            }
            Err(DiscoverError::UnreadableOverride { reason, .. }) => {
                Err(DiscoverError::UnreadableOverride {
                    path: display,
                    reason,
                })
            }
            Err(err) => Err(err),
        }
    } else {
        match discover(&effective_start, None, &RealFs) {
            Ok(workspace) => Ok(canonicalize_or_keep(&workspace)),
            Err(err) => Err(err),
        }
    }
}

pub fn workspace_start(cwd: &Path) -> PathBuf {
    let dir = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| cwd.to_path_buf());
    canonicalize_or_keep(&dir)
}

pub fn is_ci_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| value == "true")
}

pub fn is_ci() -> bool {
    is_ci_value(std::env::var("CI").ok().as_deref())
}

pub fn launcher_argv0() -> &'static str {
    "bazel"
}

pub const WORKFLOW_STARTUP_OPTS: &[&str] = &["--nohome_rc", "--nosystem_rc"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Repository,
    Pattern(String),
    Labels(Vec<String>),
    ResolvedOwners(Vec<String>),
    Count(usize),
}

pub fn describe_scope(scope: &Scope) -> String {
    match scope {
        Scope::Repository => "//...".to_owned(),
        Scope::Pattern(pattern) => pattern.clone(),
        Scope::Labels(labels) => labels.join(" "),
        Scope::ResolvedOwners(owners) => owners.join(" "),
        Scope::Count(count) => {
            if *count == 1 {
                "1 target".to_owned()
            } else {
                format!("{count} targets")
            }
        }
    }
}

pub fn operation_summary(command: &str, phase: &str, scope: &Scope) -> String {
    format!("Running {command} {phase} for {}", describe_scope(scope))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedFlag {
    pub name: String,
    pub required: Option<String>,
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForwardError {
    #[error("conflicting_option: --{flag} conflicts with required workflow policy")]
    ConflictingOption { flag: String },
    #[error("invalid_usage: --{flag} is a startup option; use dx bazel")]
    StartupOption { flag: String },
    #[error("invalid_usage: --{flag} targets the test binary; use dx bazel")]
    TestBinaryArgs { flag: String },
    #[error("internal_error: required workflow option --{flag} is missing")]
    InvalidRequiredOption { flag: String },
    #[error("internal_error: malformed required setting {option}; expected --name=value")]
    InvalidSetting { option: String },
    #[error("internal_error: {command} does not support this plan")]
    UnsupportedCommand { command: String },
}

fn flag_name(arg: &str) -> Option<String> {
    if !arg.starts_with("--") {
        return None;
    }
    let bare = &arg[2..];
    if bare.is_empty() {
        return None;
    }
    Some(bare.split('=').next().unwrap_or("").to_owned()).filter(|name| !name.is_empty())
}

pub fn is_startup_option(arg: &str) -> bool {
    const STARTUP: &[&str] = &[
        "bazelrc",
        "home_rc",
        "nohome_rc",
        "system_rc",
        "nosystem_rc",
        "output_base",
        "output_user_root",
        "host_jvm_args",
        "server_jvm_out",
    ];
    match flag_name(arg) {
        Some(name) => STARTUP.contains(&name.as_str()),
        None => false,
    }
}

pub fn is_test_binary_arg(arg: &str) -> bool {
    match flag_name(arg) {
        Some(name) => name == "test_arg",
        None => false,
    }
}

pub fn check_protected(
    user_args: &[String],
    protected: &[ProtectedFlag],
) -> Result<Vec<String>, ForwardError> {
    let mut kept = Vec::with_capacity(user_args.len());
    for arg in user_args {
        let name = flag_name(arg);
        let guard = match &name {
            Some(name) => protected.iter().find(|flag| flag.name == *name),
            None => None,
        };
        match (guard, name) {
            (Some(guard), _) => match &guard.required {
                Some(required) if arg == required => kept.push(required.clone()),
                _ if guard.allowed.iter().any(|allowed| allowed == arg) => kept.push(arg.clone()),
                Some(_) | None => {
                    return Err(ForwardError::ConflictingOption {
                        flag: guard.name.clone(),
                    });
                }
            },
            (None, _) => kept.push(arg.clone()),
        }
    }
    Ok(kept)
}

pub fn build_workflow_argv(
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    allow_test_args: bool,
) -> Result<Vec<String>, ForwardError> {
    for arg in user_options {
        if is_startup_option(arg) {
            let flag = flag_name(arg).unwrap_or_default();
            return Err(ForwardError::StartupOption { flag });
        }
        if !allow_test_args && is_test_binary_arg(arg) {
            let flag = flag_name(arg).unwrap_or_default();
            return Err(ForwardError::TestBinaryArgs { flag });
        }
    }
    let canonical = check_protected(user_options, protected)?;
    let mut argv =
        Vec::with_capacity(2 + WORKFLOW_STARTUP_OPTS.len() + canonical.len() + labels.len());
    argv.push(launcher_argv0().to_owned());
    for opt in WORKFLOW_STARTUP_OPTS {
        argv.push((*opt).to_owned());
    }
    argv.push(bazel_command.to_owned());
    argv.extend(required_options.iter().cloned());
    argv.extend(canonical);
    argv.extend(labels.iter().cloned());
    Ok(argv)
}

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_OPERATIONAL: i32 = 1;
pub const EXIT_PRE_EXEC: i32 = 2;
pub const EXIT_BROKEN_PIPE: i32 = 128 + 13;

pub fn pre_exec_code() -> i32 {
    EXIT_PRE_EXEC
}

pub fn operational_code() -> i32 {
    EXIT_OPERATIONAL
}

pub fn broken_pipe_code() -> i32 {
    EXIT_BROKEN_PIPE
}

pub fn is_broken_pipe_io(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::BrokenPipe
}

pub fn stdout_io_code(error: &io::Error) -> i32 {
    if is_broken_pipe_io(error) {
        EXIT_BROKEN_PIPE
    } else {
        EXIT_OPERATIONAL
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildStatus {
    pub code: Option<i32>,
}

pub trait Runner {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus>;

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.run(argv, cwd, env)
    }

    fn gitleaks_tool(&self) -> Option<PathBuf> {
        None
    }

    fn git_tool(&self) -> Option<PathBuf> {
        None
    }
}

pub fn spawn_output(
    argv: &[String],
    cwd: &Path,
    env: &[(&str, &str)],
    clear_env: bool,
) -> io::Result<std::process::Output> {
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut command = Command::new(OsStr::new(binary));
    command.args(args).current_dir(cwd);
    if clear_env {
        command.env_clear();
    }
    command.envs(env.iter().copied());
    command.output()
}

pub fn spawn_success(argv: &[String]) -> bool {
    let (binary, args) = match argv.split_first() {
        Some(pair) => pair,
        None => return false,
    };
    Command::new(OsStr::new(binary))
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Suffixes the host appends to an executable name.
fn suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        dx_path::host::EXECUTABLE_SUFFIXES
    } else {
        &[]
    }
}

pub fn exe_available(bin: &str) -> bool {
    spawn_success(&[bin.to_owned(), "--help".to_owned()])
        || Path::new(bin).is_file()
        || std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                dir.join(bin).is_file()
                    || suffixes()
                        .iter()
                        .any(|suffix| dir.join(format!("{bin}{suffix}")).is_file())
            })
        })
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        let output = spawn_output(argv, cwd, env, false)?;
        Ok(ChildStatus {
            code: output.status.code(),
        })
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        let output = spawn_output(argv, cwd, env, true)?;
        Ok(ChildStatus {
            code: output.status.code(),
        })
    }

    fn gitleaks_tool(&self) -> Option<PathBuf> {
        std::env::var_os("DX_GITLEAKS_BIN")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }

    fn git_tool(&self) -> Option<PathBuf> {
        std::env::var_os("DX_GIT_BIN")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
