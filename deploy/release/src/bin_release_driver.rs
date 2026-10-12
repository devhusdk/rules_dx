#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::Command;

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

fn run_with(tag: &str, approve: &str, dry: bool, dirty: bool, exists: bool) -> i32 {
    match dx_release_tools::release_run(tag, approve, dry, dirty, exists) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                None
            }
        })
}

fn tree_dirty() -> bool {
    git(&["status", "--porcelain"]).is_some_and(|out| tree_dirty_of(&out))
}

fn tag_exists(tag: &str) -> bool {
    git(&["tag", "--list", "v*"]).is_some_and(|out| tag_exists_in(&out, tag))
}

fn ls_remote_output(tag: &str) -> Option<String> {
    Command::new("git")
        .args(["ls-remote", "--tags", "origin", &format!("refs/tags/{tag}")])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                None
            }
        })
}

fn run(argv: &[String]) -> i32 {
    let tag = resolve_tag(argv).to_owned();
    let approve = approve_of(std::env::var("RELEASE_APPROVE").ok());
    let dry = dry_of(std::env::var("RELEASE_DRY_RUN").ok());
    let code = run_with(&tag, &approve, dry, tree_dirty(), tag_exists(&tag));
    if code != 0 || dry {
        return code;
    }
    match dx_release_tools::release_remote_error(&tag, ls_remote_output(&tag).as_deref()) {
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

    #[test]
    fn dry_run_dispatch_exits_zero() {
        assert_eq!(run_with("v0.0.0-dryrun", "0", true, false, false), 0);
    }

    #[test]
    fn live_dispatch_without_approval_fails() {
        assert_eq!(run_with("v1.2.3", "0", false, false, true), 1);
    }

    #[test]
    fn live_dispatch_with_placeholder_tag_fails() {
        assert_eq!(run_with("v0.0.0-dryrun", "1", false, false, true), 1);
    }

    #[test]
    fn live_dispatch_with_dirty_tree_fails() {
        assert_eq!(run_with("v1.2.3", "1", false, true, true), 1);
    }

    #[test]
    fn live_dispatch_with_missing_tag_fails() {
        assert_eq!(run_with("v1.2.3", "1", false, false, false), 1);
    }

    #[test]
    fn live_dispatch_with_approval_and_pushed_tag_succeeds() {
        assert_eq!(run_with("v1.2.3", "1", false, false, true), 0);
    }
}
