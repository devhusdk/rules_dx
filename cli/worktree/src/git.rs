use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use dx_process::lifecycle::{
    CapturePolicy, ChildOutcome, CleanupBudget, Descendants, Exit, run_command_until,
};

use super::WorktreeError;

const WRITE_TIMEOUT: Duration = Duration::from_mins(30);

const READ_TIMEOUT: Duration = Duration::from_secs(120);

fn base(cwd: &Path) -> Command {
    let mut command = dx_process::capture::git_isolated(cwd);
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}

fn command(cwd: &Path) -> Command {
    #[cfg(test)]
    {
        static SANDBOX: std::sync::OnceLock<dx_test_scratch::TempDir> = std::sync::OnceLock::new();
        let sandbox = SANDBOX.get_or_init(|| dx_test_scratch::scratch("worktree-git").unwrap());
        let mut command = base(cwd);
        dx_test_scratch::isolate_git(&mut command, sandbox.path());
        command
    }
    #[cfg(not(test))]
    base(cwd)
}

pub(super) fn run<S: AsRef<OsStr>>(cwd: &Path, args: &[S]) -> Result<String, WorktreeError> {
    let mut cmd = command(cwd);
    cmd.args(args);
    let output = dx_process::capture::run_captured(&mut cmd, WRITE_TIMEOUT)
        .map_err(|error| WorktreeError::git(args, &error.to_string()))?;
    if output.success() {
        return Ok(output.stdout_text());
    }
    Err(WorktreeError::git(args, &output.stderr_text()))
}

pub(super) fn run_until<S: AsRef<OsStr>>(
    cwd: &Path,
    args: &[S],
    timeout: Duration,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<String, WorktreeError> {
    let mut cmd = command(cwd);
    cmd.args(args);
    let capture = CapturePolicy {
        max_bytes: 64 << 20,
    };
    let outcome = run_command_until(
        &mut cmd,
        &capture,
        timeout,
        Descendants::Owned,
        CleanupBudget::STANDARD,
        stop,
    )
    .map_err(|error| WorktreeError::git(args, &format!("cannot start git: {error}")))?;
    match outcome {
        ChildOutcome::Finished {
            exit: Exit::Code(0),
            stdout,
            ..
        } => Ok(String::from_utf8_lossy(&stdout).into_owned()),
        ChildOutcome::Finished { stderr, .. } => Err(WorktreeError::git(
            args,
            String::from_utf8_lossy(&stderr).trim(),
        )),
        ChildOutcome::TimedOut => Err(match stop() {
            Some(signo) => WorktreeError::stopped(signo),
            None => WorktreeError::git(args, "timed out"),
        }),
        ChildOutcome::OutputTooLarge { .. } => Err(WorktreeError::git(args, "output too large")),
    }
}

const CONFIG_OVERRIDES: [&str; 3] = ["GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS"];

pub(super) fn own_config_command(cwd: &Path, args: &[&str]) -> Command {
    let mut cmd = command(cwd);
    for name in CONFIG_OVERRIDES {
        cmd.env_remove(name);
    }
    cmd.args(["-c", "safe.directory=*"]).args(args);
    cmd
}

pub(super) fn own_config(
    cwd: &Path,
    args: &[&str],
) -> Result<Option<dx_process::capture::Captured>, String> {
    match dx_process::capture::run_captured(&mut own_config_command(cwd, args), READ_TIMEOUT) {
        Ok(output) => Ok(Some(output)),
        Err(dx_process::capture::RunError::Spawn { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn line(cwd: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    run(cwd, args).map(|text| text.trim().to_owned())
}

pub(super) fn succeeds(cwd: &Path, args: &[&str]) -> bool {
    let mut cmd = command(cwd);
    cmd.args(args);
    dx_process::capture::run_captured(&mut cmd, WRITE_TIMEOUT).is_ok_and(|output| output.success())
}

pub(super) fn run_with_input(
    cwd: &Path,
    args: &[&str],
    input: &[u8],
) -> Result<String, WorktreeError> {
    let mut cmd = command(cwd);
    cmd.args(args);
    let output = dx_process::capture::run_captured_with_input(&mut cmd, Some(input), WRITE_TIMEOUT)
        .map_err(|error| WorktreeError::git(args, &error.to_string()))?;
    if output.success() || output.code == Some(1) {
        return Ok(output.stdout_text());
    }
    Err(WorktreeError::git(args, &output.stderr_text()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Checkout {
    pub(super) path: PathBuf,
    pub(super) branch: Option<String>,
    pub(super) detached: bool,
    pub(super) locked: bool,
    pub(super) bare: bool,
}

pub(super) fn parse_checkouts(text: &str) -> Vec<Checkout> {
    let mut found = Vec::new();
    let mut current: Option<Checkout> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(done) = current.take() {
                found.push(done);
            }
            current = Some(Checkout {
                path: PathBuf::from(path),
                branch: None,
                detached: false,
                locked: false,
                bare: false,
            });
        } else if let Some(entry) = current.as_mut() {
            if let Some(name) = line.strip_prefix("branch ") {
                entry.branch = Some(name.strip_prefix("refs/heads/").unwrap_or(name).to_owned());
            } else if line == "detached" {
                entry.detached = true;
            } else if line == "bare" {
                entry.bare = true;
            } else if line == "locked" || line.starts_with("locked ") {
                entry.locked = true;
            }
        }
    }
    if let Some(done) = current {
        found.push(done);
    }
    found
}

pub(super) fn checkouts(cwd: &Path) -> Result<Vec<Checkout>, WorktreeError> {
    Ok(parse_checkouts(&run(
        cwd,
        &["worktree", "list", "--porcelain"],
    )?))
}

pub(super) fn common_worktree(common: &Path) -> Option<PathBuf> {
    let config = common.join("config");
    let config = config.to_str()?;
    let value = line(
        common,
        &["config", "--file", config, "--get", "core.worktree"],
    )
    .ok()?;
    if value.is_empty() {
        return None;
    }
    Some(super::host::resolved(&common.join(value)))
}

pub(super) fn dirty(
    checkout: &Path,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<bool, WorktreeError> {
    let status = run_until(
        checkout,
        &["status", "--porcelain", "--untracked-files=normal"],
        READ_TIMEOUT,
        stop,
    )?;
    Ok(status
        .lines()
        .any(|line| !bazel_link(checkout, line, "?? ")))
}

pub(super) fn holds_files(
    checkout: &Path,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<bool, WorktreeError> {
    let status = run_until(
        checkout,
        &[
            "status",
            "--porcelain",
            "--ignored",
            "--untracked-files=normal",
        ],
        READ_TIMEOUT,
        stop,
    )?;
    Ok(status
        .lines()
        .any(|line| !bazel_link(checkout, line, "?? ") && !bazel_link(checkout, line, "!! ")))
}

fn bazel_link(checkout: &Path, line: &str, prefix: &str) -> bool {
    let Some(name) = line.strip_prefix(prefix) else {
        return false;
    };
    let name = name.trim_end_matches('/');
    if !name.starts_with("bazel-") {
        return false;
    }
    let workspace = checkout
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name == "bazel-bin"
        || name == "bazel-out"
        || name == "bazel-testlogs"
        || name == format!("bazel-{workspace}")
        || std::fs::symlink_metadata(checkout.join(name))
            .is_ok_and(|meta| meta.file_type().is_symlink())
}

pub(super) fn operation_in_progress(
    checkout: &Path,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<bool, WorktreeError> {
    let dir = run_until(
        checkout,
        &["rev-parse", "--absolute-git-dir"],
        READ_TIMEOUT,
        stop,
    )?;
    let dir = PathBuf::from(dir.trim());
    Ok([
        "rebase-merge",
        "rebase-apply",
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_LOG",
        "sequencer",
    ]
    .iter()
    .any(|name| dir.join(name).exists()))
}

pub(super) fn unpushed(
    checkout: &Path,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<u64, WorktreeError> {
    let text = run_until(
        checkout,
        &["rev-list", "--count", "HEAD", "--not", "--remotes"],
        READ_TIMEOUT,
        stop,
    )?;
    let args = ["rev-list", "--count", "HEAD", "--not", "--remotes"];
    text.trim().parse().map_err(|error| {
        WorktreeError::git(&args, &format!("unreadable commit count {text:?}: {error}"))
    })
}

pub(super) fn remote_heads(
    repo: &Path,
    stem: &str,
    stop: &dyn Fn() -> Option<i32>,
) -> Result<std::collections::HashSet<String>, WorktreeError> {
    let exact = format!("refs/heads/{stem}");
    let suffixed = format!("refs/heads/{stem}-*");
    let text = run_until(
        repo,
        &["ls-remote", "origin", &exact, &suffixed],
        READ_TIMEOUT,
        stop,
    )?;
    Ok(text
        .lines()
        .filter_map(|row| row.split_once('\t'))
        .filter_map(|(_, name)| name.strip_prefix("refs/heads/"))
        .map(str::to_owned)
        .collect())
}

pub(super) fn current_branch(checkout: &Path) -> Option<String> {
    line(checkout, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .filter(|name| !name.is_empty())
}

pub(super) fn default_base(repo: &Path) -> Result<String, WorktreeError> {
    if let Ok(name) = line(
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) && !name.is_empty()
        && ref_exists(repo, &name)
    {
        return Ok(name);
    }
    for candidate in ["origin/main", "origin/master"] {
        if succeeds(repo, &["rev-parse", "--verify", "--quiet", candidate]) {
            return Ok(candidate.to_owned());
        }
    }
    Err(WorktreeError::refused(
        "no_default_branch",
        "cannot find the default branch: origin/HEAD, origin/main and origin/master are all missing; pass --create=<ref>",
    ))
}

pub(super) fn ref_exists(repo: &Path, name: &str) -> bool {
    succeeds(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{commit}}"),
        ],
    )
}

pub(super) fn branch_behind(repo: &Path, name: &str, base: &str) -> bool {
    let tip = line(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}^{{commit}}"),
        ],
    );
    let head = line(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
    );
    match (tip, head) {
        (Ok(tip), Ok(head)) if tip != head => {
            succeeds(repo, &["merge-base", "--is-ancestor", &tip, &head])
        }
        _ => false,
    }
}

pub(super) fn branch_exists(repo: &Path, branch: &str) -> bool {
    succeeds(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
}

pub(super) fn lock_digest(repo: &Path, reference: &str) -> Option<String> {
    line(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}:MODULE.bazel.lock"),
        ],
    )
    .ok()
    .filter(|id| !id.is_empty())
}

pub(super) fn admin_dir(checkout: &Path) -> Option<PathBuf> {
    line(checkout, &["rev-parse", "--absolute-git-dir"])
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

pub(super) fn is_ancestor(repo: &Path, rev: &str, base: &str) -> bool {
    succeeds(repo, &["merge-base", "--is-ancestor", rev, base])
}

pub(super) fn tracks_own_remote(repo: &Path, branch: &str) -> bool {
    line(
        repo,
        &[
            "for-each-ref",
            "--format=%(upstream)",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok_and(|text| text == format!("refs/remotes/origin/{branch}"))
}

pub(super) fn upstream_gone(repo: &Path, branch: &str) -> bool {
    line(
        repo,
        &[
            "for-each-ref",
            "--format=%(upstream:track)",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok_and(|text| text == "[gone]")
}
