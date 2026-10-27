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
use std::path::PathBuf;

use dx_release_tools::candidate::{self, Candidate};
use dx_release_tools::GitState;

const DRY_RUN_TAG: &str = "v0.0.0-dryrun";

fn resolve_tag(argv: &[String]) -> &str {
    argv.get(1).map(String::as_str).unwrap_or(DRY_RUN_TAG)
}

fn resolve_commit(argv: &[String]) -> Option<&str> {
    argv.get(2).map(String::as_str)
}

fn approve_of(value: Option<String>) -> String {
    value.unwrap_or_else(|| "0".to_owned())
}

fn dry_of(value: Option<String>) -> bool {
    value.as_deref().unwrap_or("1") != "0"
}

fn workspace_dir_of(value: Option<OsString>) -> Option<PathBuf> {
    value
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_absolute())
}

fn run_with(
    tag: &str,
    commit: Option<&str>,
    approve: &str,
    dry: bool,
    git_state: impl FnOnce() -> Result<GitState, String>,
) -> i32 {
    match dx_release_tools::release_run(tag, commit, approve, dry, git_state) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn remote_with(candidate: &Candidate, ls_remote: Result<String, String>) -> i32 {
    match dx_release_tools::release_remote_error(candidate, ls_remote) {
        Ok(sha) => {
            println!(
                "release: remote tag {} verified at {sha} ({})",
                candidate.tag(),
                candidate.remote()
            );
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let tag = resolve_tag(argv).to_owned();
    let commit = resolve_commit(argv).map(str::to_owned);
    let approve = approve_of(std::env::var("RELEASE_APPROVE").ok());
    let dry = dry_of(std::env::var("RELEASE_DRY_RUN").ok());
    let dir = workspace_dir_of(std::env::var_os("BUILD_WORKSPACE_DIRECTORY"));
    let code = run_with(&tag, commit.as_deref(), &approve, dry, || {
        candidate::read_git_state(dir.as_deref(), &tag)
    });
    if code != 0 || dry {
        return code;
    }
    match dx_release_tools::release_candidate(&tag, commit.as_deref()) {
        Ok(candidate) => {
            let output =
                candidate::ls_remote_output(dir.as_deref(), candidate.remote(), &candidate);
            remote_with(&candidate, output)
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

    const SHA: &str = "a948904f2f0f479b8f8197694b30184b0d2ed1c1";
    const OTHER: &str = "b948904f2f0f479b8f8197694b30184b0d2ed1c1";

    #[test]
    fn missing_tag_arg_falls_back_to_the_dry_run_placeholder() {
        assert_eq!(resolve_tag(&[]), DRY_RUN_TAG);
        assert_eq!(resolve_tag(&["release_driver".to_owned()]), DRY_RUN_TAG);
        assert_eq!(resolve_commit(&["release_driver".to_owned()]), None);
    }

    #[test]
    fn explicit_tag_and_commit_args_reach_release_dispatch() {
        let argv = [
            "release_driver".to_owned(),
            "v1.2.3".to_owned(),
            SHA.to_owned(),
        ];
        assert_eq!(resolve_tag(&argv), "v1.2.3");
        assert_eq!(resolve_commit(&argv), Some(SHA));
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

    fn git_unread() -> Result<GitState, String> {
        panic!("git state read before the dry-run, approval, or placeholder gate")
    }

    #[test]
    #[should_panic(expected = "git state read")]
    fn live_dispatch_reads_git_after_the_gates() {
        let _ = run_with("v1.2.3", Some(SHA), "1", false, git_unread);
    }

    fn git_ok(tree_dirty: bool, head: &str, tag: Option<&str>) -> Result<GitState, String> {
        Ok(GitState {
            tree_dirty,
            head: head.to_owned(),
            tag_commit: tag.map(str::to_owned),
        })
    }

    #[test]
    fn dry_run_dispatch_exits_zero() {
        assert_eq!(run_with("v0.0.0-dryrun", None, "0", true, git_unread), 0);
    }

    #[test]
    fn live_dispatch_without_approval_fails() {
        assert_eq!(run_with("v1.2.3", Some(SHA), "0", false, git_unread), 1);
    }

    #[test]
    fn live_dispatch_with_placeholder_tag_fails() {
        assert_eq!(
            run_with("v0.0.0-dryrun", Some(SHA), "1", false, git_unread),
            1
        );
    }

    #[test]
    fn live_dispatch_without_commit_fails() {
        assert_eq!(run_with("v1.2.3", None, "1", false, git_unread), 1);
    }

    #[test]
    fn live_dispatch_with_dirty_tree_fails() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || git_ok(
                true,
                SHA,
                Some(SHA)
            )),
            1
        );
    }

    #[test]
    fn live_dispatch_with_stale_head_fails() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || git_ok(
                false,
                OTHER,
                Some(SHA)
            )),
            1
        );
    }

    #[test]
    fn live_dispatch_with_missing_tag_fails() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || git_ok(false, SHA, None)),
            1
        );
    }

    #[test]
    fn live_dispatch_with_tag_elsewhere_fails() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || git_ok(
                false,
                SHA,
                Some(OTHER)
            )),
            1
        );
    }

    #[test]
    fn live_dispatch_with_approval_and_pushed_tag_succeeds() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || git_ok(
                false,
                SHA,
                Some(SHA)
            )),
            0
        );
    }

    #[test]
    fn live_dispatch_with_git_error_fails() {
        assert_eq!(
            run_with("v1.2.3", Some(SHA), "1", false, || Err(
                "release: cannot read git state: x".to_owned()
            )),
            1
        );
    }

    #[test]
    fn live_dispatch_remote_check_compares_the_candidate_commit() {
        let candidate = Candidate::new("1.2.3", SHA).expect("candidate");
        assert_eq!(
            remote_with(&candidate, Ok(format!("{SHA}\trefs/tags/v1.2.3\n"))),
            0
        );
        assert_eq!(
            remote_with(&candidate, Ok(format!("{OTHER}\trefs/tags/v1.2.3\n"))),
            1
        );
        assert_eq!(remote_with(&candidate, Ok(String::new())), 1);
        assert_eq!(remote_with(&candidate, Err("fatal: offline".to_owned())), 1);
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
}
