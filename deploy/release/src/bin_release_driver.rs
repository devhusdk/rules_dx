#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dx_release_tools::GitState;

const DRY_RUN_TAG: &str = "v0.0.0-dryrun";

fn resolve_tag(argv: &[String]) -> &str {
    argv.get(1).map(String::as_str).unwrap_or(DRY_RUN_TAG)
}

fn approve_of(value: Option<String>) -> String {
    value.unwrap_or_else(|| "0".to_owned())
}

fn dry_of(value: Option<String>) -> bool {
    value.as_deref().unwrap_or("1") != "0"
}

fn tree_dirty_of(status: &str) -> bool {
    !status.trim().is_empty()
}

fn tag_exists_in(tag_list: &str, tag: &str) -> bool {
    tag_list.lines().any(|line| line.trim() == tag)
}

fn workspace_dir_of(value: Option<OsString>) -> Option<PathBuf> {
    value
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_absolute())
}

fn git_output_of(args: &[&str], output: io::Result<Output>) -> Result<String, String> {
    let command = format!("git {}", args.join(" "));
    match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let detail = if stderr.is_empty() {
                out.status.to_string()
            } else {
                stderr
            };
            Err(format!(
                "release: cannot read git state: {command} failed: {detail}"
            ))
        }
        Err(err) => Err(format!(
            "release: cannot read git state: {command} failed: {err}"
        )),
    }
}

fn git_state_of(
    status: Result<String, String>,
    tags: Result<String, String>,
    tag: &str,
) -> Result<GitState, String> {
    let status = status?;
    let tags = tags?;
    Ok(GitState {
        tree_dirty: tree_dirty_of(&status),
        tag_exists: tag_exists_in(&tags, tag),
    })
}

fn run_with(
    tag: &str,
    approve: &str,
    dry: bool,
    git_state: impl FnOnce() -> Result<GitState, String>,
) -> i32 {
    match dx_release_tools::release_run(tag, approve, dry, git_state) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn git(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    git_output_of(args, cmd.output())
}

fn ls_remote_output(dir: Option<&Path>, tag: &str) -> Option<String> {
    let reference = format!("refs/tags/{tag}");
    git(dir, &["ls-remote", "--tags", "origin", &reference]).ok()
}

fn run(argv: &[String]) -> i32 {
    let tag = resolve_tag(argv).to_owned();
    let approve = approve_of(std::env::var("RELEASE_APPROVE").ok());
    let dry = dry_of(std::env::var("RELEASE_DRY_RUN").ok());
    let dir = workspace_dir_of(std::env::var_os("BUILD_WORKSPACE_DIRECTORY"));
    let code = run_with(&tag, &approve, dry, || {
        git_state_of(
            git(dir.as_deref(), &["status", "--porcelain"]),
            git(dir.as_deref(), &["tag", "--list", "v*"]),
            &tag,
        )
    });
    if code != 0 || dry {
        return code;
    }
    match dx_release_tools::release_remote_error(
        &tag,
        ls_remote_output(dir.as_deref(), &tag).as_deref(),
    ) {
        Ok(sha) => {
            println!("release: remote tag {tag} verified at {sha} (git ls-remote --tags origin)");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tag_arg_falls_back_to_the_dry_run_placeholder() {
        assert_eq!(resolve_tag(&[]), DRY_RUN_TAG);
        assert_eq!(resolve_tag(&["release_driver".to_owned()]), DRY_RUN_TAG);
    }

    #[test]
    fn explicit_tag_arg_reaches_release_dispatch() {
        assert_eq!(
            resolve_tag(&["release_driver".to_owned(), "v1.2.3".to_owned()]),
            "v1.2.3"
        );
    }

    #[test]
    fn approval_defaults_to_unapproved() {
        assert_eq!(approve_of(None), "0");
        assert_eq!(approve_of(Some("1".to_owned())), "1");
    }

    #[test]
    fn dry_run_defaults_to_dry() {
        assert!(dry_of(None));
        assert!(dry_of(Some("1".to_owned())));
        assert!(!dry_of(Some("0".to_owned())));
    }

    #[test]
    fn whitespace_only_status_is_a_clean_tree() {
        assert!(!tree_dirty_of(""));
        assert!(!tree_dirty_of("  \n"));
        assert!(tree_dirty_of(" M src/lib.rs\n"));
    }

    #[test]
    fn tag_match_is_exact_per_line() {
        assert!(tag_exists_in("v1.2.3\nv1.2.30\n", "v1.2.3"));
        assert!(!tag_exists_in("v1.2.30\n", "v1.2.3"));
        assert!(!tag_exists_in("", "v1.2.3"));
    }

    fn git_unread() -> Result<GitState, String> {
        panic!("git state read before the dry-run, approval, or placeholder gate")
    }

    #[test]
    #[should_panic(expected = "git state read")]
    fn live_dispatch_reads_git_after_the_gates() {
        let _ = run_with("v1.2.3", "1", false, git_unread);
    }

    fn git_ok(tree_dirty: bool, tag_exists: bool) -> Result<GitState, String> {
        Ok(GitState {
            tree_dirty,
            tag_exists,
        })
    }

    #[test]
    fn dry_run_dispatch_exits_zero() {
        assert_eq!(run_with("v0.0.0-dryrun", "0", true, git_unread), 0);
    }

    #[test]
    fn live_dispatch_without_approval_fails() {
        assert_eq!(run_with("v1.2.3", "0", false, git_unread), 1);
    }

    #[test]
    fn live_dispatch_with_placeholder_tag_fails() {
        assert_eq!(run_with("v0.0.0-dryrun", "1", false, git_unread), 1);
    }

    #[test]
    fn live_dispatch_with_dirty_tree_fails() {
        assert_eq!(run_with("v1.2.3", "1", false, || git_ok(true, true)), 1);
    }

    #[test]
    fn live_dispatch_with_missing_tag_fails() {
        assert_eq!(run_with("v1.2.3", "1", false, || git_ok(false, false)), 1);
    }

    #[test]
    fn live_dispatch_with_approval_and_pushed_tag_succeeds() {
        assert_eq!(run_with("v1.2.3", "1", false, || git_ok(false, true)), 0);
    }

    #[test]
    fn live_dispatch_with_git_error_fails() {
        assert_eq!(
            run_with("v1.2.3", "1", false, || Err(
                "release: cannot read git state: x".to_owned()
            )),
            1
        );
    }

    #[test]
    fn workspace_dir_must_be_absolute() {
        assert_eq!(workspace_dir_of(None), None);
        assert_eq!(workspace_dir_of(Some(OsString::new())), None);
        assert_eq!(workspace_dir_of(Some(OsString::from("repo"))), None);
        let absolute = std::env::temp_dir();
        assert_eq!(
            workspace_dir_of(Some(absolute.clone().into_os_string())),
            Some(absolute)
        );
    }

    #[cfg(unix)]
    #[test]
    fn git_failure_is_an_error_not_clean() {
        use std::os::unix::process::ExitStatusExt;
        let failed = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: b"fatal: not a git repository\n".to_vec(),
        };
        assert_eq!(
            git_output_of(&["status", "--porcelain"], Ok(failed)),
            Err("release: cannot read git state: git status --porcelain failed: fatal: not a git repository".to_owned())
        );
        let silent = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let err = git_output_of(&["tag"], Ok(silent)).expect_err("silent failure");
        assert!(err.starts_with("release: cannot read git state: git tag failed: "));
        assert!(err.contains("1"));
        let spawn = io::Error::new(io::ErrorKind::NotFound, "no git");
        assert_eq!(
            git_output_of(&["tag"], Err(spawn)),
            Err("release: cannot read git state: git tag failed: no git".to_owned())
        );
        let ok = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: b" M src/lib.rs\n".to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(
            git_output_of(&["status"], Ok(ok)),
            Ok(" M src/lib.rs\n".to_owned())
        );
    }

    #[test]
    fn git_state_of_propagates_first_error_and_parses_ok() {
        let state = git_state_of(Ok(" M a\n".to_owned()), Ok("v1.2.3\n".to_owned()), "v1.2.3")
            .expect("state");
        assert!(state.tree_dirty);
        assert!(state.tag_exists);
        let clean =
            git_state_of(Ok(String::new()), Ok("v1.2.30\n".to_owned()), "v1.2.3").expect("state");
        assert!(!clean.tree_dirty);
        assert!(!clean.tag_exists);
        assert_eq!(
            git_state_of(Err("first".to_owned()), Err("second".to_owned()), "v1.2.3").err(),
            Some("first".to_owned())
        );
        assert_eq!(
            git_state_of(Ok(String::new()), Err("second".to_owned()), "v1.2.3").err(),
            Some("second".to_owned())
        );
    }
}
