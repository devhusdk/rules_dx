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
use std::sync::atomic::{AtomicI32, Ordering};

pub mod groups;
pub mod lifecycle;
pub mod procs;
pub mod rc;
mod runner;

pub use rc::{
    build_verb_options, clear_share, configure, configure_real, current, default_disk_cache,
    repository_caches, set_share, share, user_cache_dir, LocalShare, RcPolicy,
    DISK_CACHE_GC_MAX_AGE, DISK_CACHE_GC_MAX_SIZE, PRESET_LOCAL_RESOURCES,
};
pub use runner::BinaryRunner;

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

/// Picks an absolute `bazel run` directory value, else `cwd`, canonicalized.
pub fn bazel_run_dir(value: Option<&std::ffi::OsStr>, cwd: &Path) -> PathBuf {
    let dir = value
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| cwd.to_path_buf());
    canonicalize_or_keep(&dir)
}

pub fn workspace_start(cwd: &Path) -> PathBuf {
    bazel_run_dir(
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").as_deref(),
        cwd,
    )
}

/// Returns the user's current directory, honoring `bazel run`.
pub fn current_directory() -> std::io::Result<PathBuf> {
    Ok(bazel_run_dir(
        std::env::var_os("BUILD_WORKING_DIRECTORY").as_deref(),
        &std::env::current_dir()?,
    ))
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

pub const QUALIFIED_STARTUP_OPTIONS: &[&str] = &["output_base", "output_user_root"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartupOptionError {
    #[error("missing value: --bazel-startup-option --{flag} needs =<path>")]
    MissingValue { flag: String },
    #[error(
        "unsupported startup option: --{flag}; want --output_base=<path>|--output_user_root=<path>"
    )]
    UnsupportedStartup { flag: String },
}

/// Replaces the user and password of every `scheme://user:pass@host` in `text`.
pub fn redact_url_credentials(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("://") {
        let (head, tail) = rest.split_at(start + 3);
        redacted.push_str(head);
        let authority_end = tail
            .find(|cell: char| matches!(cell, '/' | '?' | '#') || cell.is_whitespace())
            .unwrap_or(tail.len());
        match tail[..authority_end].rfind('@') {
            Some(at) => {
                redacted.push_str("(credentials)");
                rest = &tail[at..];
            }
            None => rest = tail,
        }
    }
    redacted.push_str(rest);
    redacted
}

pub fn validate_startup_option(token: &str) -> Result<String, StartupOptionError> {
    let Some(bare) = token.strip_prefix("--") else {
        let name = token.split('=').next().unwrap_or(token);
        return Err(StartupOptionError::UnsupportedStartup {
            flag: redact_url_credentials(name),
        });
    };
    let (name, value) = match bare.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (bare, None),
    };
    if name.is_empty() || !QUALIFIED_STARTUP_OPTIONS.contains(&name) {
        return Err(StartupOptionError::UnsupportedStartup {
            flag: name.to_owned(),
        });
    }
    match value {
        Some(path) if !path.is_empty() => Ok(token.to_owned()),
        _ => Err(StartupOptionError::MissingValue {
            flag: name.to_owned(),
        }),
    }
}

pub fn startup_argv(startup_options: &[String]) -> Vec<String> {
    startup_argv_with(current(), startup_options)
}

/// The launcher and startup flags under an explicit rc policy.
pub fn startup_argv_with(policy: &RcPolicy, startup_options: &[String]) -> Vec<String> {
    let opts = policy.startup_opts();
    let mut argv = Vec::with_capacity(1 + opts.len() + startup_options.len());
    argv.push(launcher_argv0().to_owned());
    for opt in opts {
        argv.push((*opt).to_owned());
    }
    argv.extend(startup_options.iter().cloned());
    argv
}

/// Startup flags and per-call options that every dx Bazel call starts with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BazelHead {
    pub startup: Vec<String>,
    pub options: Vec<String>,
}

impl BazelHead {
    /// The launcher, startup flags, verb, verb options, then the head options.
    pub fn argv(&self, verb: &str) -> Vec<String> {
        let mut argv = startup_argv(&self.startup);
        argv.push(verb.to_owned());
        argv.extend(build_verb_options(verb, &self.options));
        argv.extend(self.options.iter().cloned());
        argv
    }
}

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
    #[error("invalid_usage: --{flag} is a startup option; use native bazel")]
    StartupOption { flag: String },
    #[error("invalid_usage: --{flag} targets the test binary; use native bazel")]
    TestBinaryArgs { flag: String },
    #[error("internal_error: required workflow option --{flag} is missing")]
    InvalidRequiredOption { flag: String },
    #[error("internal_error: malformed required setting {option}; expected --name=value")]
    InvalidSetting { option: String },
    #[error("internal_error: {command} does not support this plan")]
    UnsupportedCommand { command: String },
}

/// Returns the option name of a `--name` or `--name=value` token.
pub fn flag_name(arg: &str) -> Option<String> {
    if !arg.starts_with("--") {
        return None;
    }
    let bare = &arg[2..];
    if bare.is_empty() {
        return None;
    }
    Some(bare.split('=').next().unwrap_or("").to_owned()).filter(|name| !name.is_empty())
}

/// Bazel startup options rejected after `--`.
pub const STARTUP_OPTIONS: &[&str] = &[
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

pub fn is_startup_option(arg: &str) -> bool {
    match flag_name(arg) {
        Some(name) => STARTUP_OPTIONS.contains(&name.as_str()),
        None => false,
    }
}

pub fn is_test_binary_arg(arg: &str) -> bool {
    match flag_name(arg) {
        Some(name) => name == "test_arg",
        None => false,
    }
}

fn accepts_test_binary_arg(bazel_command: &str) -> bool {
    matches!(bazel_command, "test" | "coverage")
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

/// The rc policy and slot share a workflow argv is built under.
#[derive(Debug, Clone, Copy)]
pub struct ArgvPolicy<'a> {
    pub rc: &'a RcPolicy,
    pub share: Option<&'a LocalShare>,
}

/// Builds a workflow argv; required options follow user options so dx values win.
pub fn build_workflow_argv(
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, ForwardError> {
    let share = rc::share();
    build_workflow_argv_with(
        ArgvPolicy {
            rc: current(),
            share: share.as_ref(),
        },
        bazel_command,
        user_options,
        required_options,
        protected,
        labels,
        startup_options,
    )
}

/// The workflow argv under an explicit rc policy; required options follow user options.
pub fn build_workflow_argv_with(
    policy: ArgvPolicy<'_>,
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, ForwardError> {
    for arg in user_options {
        if is_startup_option(arg) {
            let flag = flag_name(arg).unwrap_or_default();
            return Err(ForwardError::StartupOption { flag });
        }
        if is_test_binary_arg(arg) && !accepts_test_binary_arg(bazel_command) {
            let flag = flag_name(arg).unwrap_or_default();
            return Err(ForwardError::TestBinaryArgs { flag });
        }
    }
    let canonical = check_protected(user_options, protected)?;
    let verb_options = policy
        .rc
        .verb_options(bazel_command, policy.share, &canonical);
    let mut argv = startup_argv_with(policy.rc, startup_options);
    argv.reserve(1 + verb_options.len() + required_options.len() + canonical.len() + labels.len());
    argv.push(bazel_command.to_owned());
    argv.extend(verb_options);
    argv.extend(canonical);
    argv.extend(required_options.iter().cloned());
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

static INTERRUPT: AtomicI32 = AtomicI32::new(0);

/// Records a stop signal; safe inside a signal handler.
pub fn record_interrupt(signo: i32) {
    INTERRUPT.store(signo, Ordering::SeqCst);
}

/// The stop signal recorded so far, if any.
pub fn interrupted() -> Option<i32> {
    match INTERRUPT.load(Ordering::SeqCst) {
        0 => None,
        signo => Some(signo),
    }
}

/// The exit code for a run stopped by `signo`.
pub fn interrupted_code(signo: i32) -> i32 {
    128 + signo
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
    /// The signal that ended the child, when it died by a known signal.
    pub signal: Option<i32>,
}

impl ChildStatus {
    /// A child that exited with `code`.
    pub fn exited(code: i32) -> Self {
        Self {
            code: Some(code),
            signal: None,
        }
    }

    /// A child that died by `signal`; `None` or 0 means the signal is unknown.
    pub fn signalled(signal: Option<i32>) -> Self {
        Self {
            code: None,
            signal: signal.filter(|signo| *signo != 0),
        }
    }
}

impl From<lifecycle::Exit> for ChildStatus {
    fn from(exit: lifecycle::Exit) -> Self {
        match exit {
            lifecycle::Exit::Code(code) => Self::exited(code),
            lifecycle::Exit::Signal(signo) => Self::signalled(Some(signo)),
        }
    }
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

    /// Runs one child in its own process group and stops that tree once `withdraw` holds; `None` means withdrawn.
    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        if withdraw() {
            return Ok(None);
        }
        self.run(argv, cwd, env).map(Some)
    }

    /// Names the absolute path one pinned-tool variable points at.
    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        let _ = var;
        None
    }

    /// Reads the first line `path --version` prints; `None` when it cannot run.
    fn tool_version(&self, path: &Path) -> Option<String> {
        let _ = path;
        None
    }

    /// Reads the free bytes on the disk holding `path`; `None` when it cannot tell.
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        let _ = path;
        None
    }
}

/// Free and total bytes on one disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskSpace {
    pub free: u64,
    pub total: u64,
}

/// The disk space from `available total` counts in units of `unit` bytes.
pub fn space_from_counts(available: &str, total: &str, unit: u64) -> Option<DiskSpace> {
    let available: u64 = available.trim().parse().ok()?;
    let total: u64 = total.trim().parse().ok()?;
    if total == 0 {
        return None;
    }
    Some(DiskSpace {
        free: available.checked_mul(unit)?,
        total: total.checked_mul(unit)?,
    })
}

/// The disk space from `df -Pk` output.
pub fn space_from_df(text: &str) -> Option<DiskSpace> {
    let fields: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
    space_from_counts(fields.get(3)?, fields.get(1)?, 1024)
}

/// Reads the free and total bytes on the disk holding `path` or its nearest existing ancestor.
pub fn disk_space(path: &Path) -> Option<DiskSpace> {
    let dir = path.ancestors().find(|ancestor| ancestor.is_dir())?;
    dir_disk_space(dir)
}

#[cfg(unix)]
fn dir_disk_space(dir: &Path) -> Option<DiskSpace> {
    space_from_df(&probe("df", [OsStr::new("-Pk"), dir.as_os_str()])?)
}

#[cfg(not(unix))]
fn dir_disk_space(dir: &Path) -> Option<DiskSpace> {
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            free_to_caller: *mut u64,
            total: *mut u64,
            total_free: *mut u64,
        ) -> i32;
    }

    let mut wide: Vec<u16> = dir.as_os_str().encode_wide().collect();
    if !matches!(wide.last(), Some(&c) if c == u16::from(b'\\') || c == u16::from(b'/')) {
        wide.push(u16::from(b'\\'));
    }
    wide.push(0);
    let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) };
    if ok == 0 || total == 0 {
        return None;
    }
    Some(DiskSpace { free, total })
}

/// Reads one pinned-tool variable, refusing a relative path.
pub fn pinned_env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// Reads the first line `path --version` prints with an empty environment.
pub fn version_line(path: &Path) -> Option<String> {
    let argv = [path.to_string_lossy().into_owned(), "--version".to_owned()];
    let output = spawn_output(&argv, Path::new("."), &[], true).ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_owned())
}

/// Gives the owner full access to `path` and everything under it, never following a symlink.
pub fn make_writable(path: &Path) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        permissions.set_mode(permissions.mode() | 0o700);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
    }
    let _ = std::fs::set_permissions(path, permissions);
    if metadata.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                make_writable(&entry.path());
            }
        }
    }
}

/// Removes a directory tree, including read-only entries, and reports whether it is gone.
pub fn remove_tree(path: &Path) -> bool {
    if std::fs::remove_dir_all(path).is_err() {
        make_writable(path);
        let _ = std::fs::remove_dir_all(path);
    }
    !path.exists()
}

/// Runs one host query with the inherited environment and returns its stdout only when it succeeds.
pub fn probe<I, S>(program: &str, args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
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

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
#[path = "lib_tests.rs"]
mod lib_tests;
