use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use dx_process::lifecycle::{
    run_command_until, CapturePolicy, ChildOutcome, CleanupBudget, Descendants, Exit,
};

use super::WorktreeError;

const GIT_ENV: [&str; 4] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
];

/// Bound for the per-slot status reads. A timeout keeps the slot.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

fn command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    for name in GIT_ENV {
        command.env_remove(name);
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    // Tests must not see the developer's global hooks, `insteadOf` or signing config.
    #[cfg(test)]
    super::tests::isolate(&mut command);
    command
}

/// Runs git and returns stdout, or the failure with its stderr.
pub(super) fn run(cwd: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    let output = command(cwd)
        .args(args)
        .output()
        .map_err(|error| WorktreeError::git(args, &format!("cannot start git: {error}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(WorktreeError::git(
        args,
        String::from_utf8_lossy(&output.stderr).trim(),
    ))
}

/// Like `run`, for reads only: stops the child once `stop` returns a signal
/// and bounds it by `timeout`. A stop is `WorktreeError::stopped`; a timeout
/// is a git failure, never a result.
pub(super) fn run_until(
    cwd: &Path,
    args: &[&str],
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

/// Runs git and returns trimmed stdout.
pub(super) fn line(cwd: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    run(cwd, args).map(|text| text.trim().to_owned())
}

/// Reports whether git exits zero.
pub(super) fn succeeds(cwd: &Path, args: &[&str]) -> bool {
    command(cwd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Runs git with data on stdin and returns stdout.
pub(super) fn run_with_input(
    cwd: &Path,
    args: &[&str],
    input: &[u8],
) -> Result<String, WorktreeError> {
    use std::io::Write as _;
    let mut child = command(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| WorktreeError::git(args, &format!("cannot start git: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| WorktreeError::git(args, &error.to_string()))?;
    if output.status.success() || output.status.code() == Some(1) {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(WorktreeError::git(
        args,
        String::from_utf8_lossy(&output.stderr).trim(),
    ))
}

/// One entry of `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Checkout {
    pub(super) path: PathBuf,
    pub(super) branch: Option<String>,
    pub(super) detached: bool,
    pub(super) locked: bool,
    pub(super) bare: bool,
}

/// Parses `git worktree list --porcelain` output.
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

/// Every checkout of the repository, main checkout first.
pub(super) fn checkouts(cwd: &Path) -> Result<Vec<Checkout>, WorktreeError> {
    Ok(parse_checkouts(&run(
        cwd,
        &["worktree", "list", "--porcelain"],
    )?))
}

/// The `core.worktree` checkout recorded in a common git directory.
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

/// Whether the checkout has changes git would commit or untracked files.
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
    Ok(status.lines().any(|line| !line.starts_with("?? bazel-")))
}

/// How many commits on HEAD are on no remote.
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
    Ok(text.trim().parse().unwrap_or(0))
}

/// Branches on origin named `stem` or `stem-*`, read from the remote itself.
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

/// The branch the checkout has switched to, if any.
pub(super) fn current_branch(checkout: &Path) -> Option<String> {
    line(checkout, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .filter(|name| !name.is_empty())
}

/// The remote-tracking ref of the default branch, such as `origin/main`.
pub(super) fn default_base(repo: &Path) -> Result<String, WorktreeError> {
    if let Ok(name) = line(
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        if !name.is_empty() {
            return Ok(name);
        }
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

/// Whether the ref resolves to an object.
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

/// Whether local branch `name` is strictly behind `base`: its tip is an
/// ancestor of `base` but not equal to it, so it holds no unmerged work and
/// would start an agent on an old commit.
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

/// Whether a local branch with this name exists.
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

/// The blob id of `MODULE.bazel.lock` at a ref, if it has one.
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

/// The checkout's own git directory, such as `<repo>/.git/worktrees/<name>`.
pub(super) fn admin_dir(checkout: &Path) -> Option<PathBuf> {
    line(checkout, &["rev-parse", "--absolute-git-dir"])
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Whether `rev` is reachable from `base`.
pub(super) fn is_ancestor(repo: &Path, rev: &str, base: &str) -> bool {
    succeeds(repo, &["merge-base", "--is-ancestor", rev, base])
}

/// Whether the branch tracks origin/<branch>.
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

/// Whether the branch tracks a remote branch that no longer exists.
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
