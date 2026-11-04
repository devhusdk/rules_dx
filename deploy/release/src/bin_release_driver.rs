#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_release_tools::candidate::{self, Candidate};
use dx_release_tools::ledger::{sync_assets, ReleaseBackend};
use dx_release_tools::{ledger_exit, render_ledger, GitState, OpRecord, OpState};

const DRY_RUN_TAG: &str = "v0.0.0-dryrun";
const DEFAULT_REPO: &str = "devhusdk/rules_dx";

/// Releases API path for the repo the release job runs in, so forks work.
fn release_api(repo: Option<String>) -> String {
    let repo = repo.filter(|r| !r.is_empty());
    format!(
        "repos/{}/releases/tags",
        repo.as_deref().unwrap_or(DEFAULT_REPO)
    )
}

fn resolve_assets(argv: &[String]) -> &[String] {
    argv.get(3..).unwrap_or(&[])
}

struct GhBackend {
    program: String,
    token: Option<String>,
    api: String,
}

impl GhBackend {
    fn call(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|err| format!("cannot run gh: {err}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(scrub(stderr.trim(), self.token.as_deref()))
    }
}

fn scrub(text: &str, secret: Option<&str>) -> String {
    match secret {
        Some(secret) if !secret.is_empty() => text.replace(secret, "***"),
        _ => text.to_owned(),
    }
}

impl ReleaseBackend for GhBackend {
    fn view(&self, tag: &str) -> Result<String, String> {
        self.call(&["api", &format!("{}/{tag}", self.api)])
    }

    fn upload(&self, tag: &str, path: &str) -> Result<String, String> {
        self.call(&["release", "upload", tag, path])
    }
}

fn stage_assets(
    assets: &[String],
) -> Result<(Vec<(String, String)>, BTreeMap<String, String>), String> {
    let mut staged = Vec::new();
    let mut paths = BTreeMap::new();
    for asset in assets {
        let name = Path::new(asset)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("release: invalid asset path {asset}"))?
            .to_owned();
        let sha = dx_digest::sha256_file_hex(Path::new(asset))
            .map_err(|err| format!("release: cannot hash {asset}: {err}"))?;
        staged.push((name.clone(), sha));
        paths.insert(name, asset.clone());
    }
    Ok((staged, paths))
}

fn ledger_with(
    backend: &dyn ReleaseBackend,
    tag: &str,
    assets: &[String],
    approved: bool,
) -> (String, i32) {
    let mut records = match stage_assets(assets) {
        Ok((staged, paths)) => sync_assets(backend, tag, &staged, &paths, approved),
        Err(err) => vec![OpRecord::new("asset-stage", tag, OpState::Failed, &err)],
    };
    records.push(OpRecord::new(
        "publish",
        tag,
        OpState::NotAuthorized,
        "the owner publishes the draft release by hand",
    ));
    (render_ledger(&records), ledger_exit(&records))
}

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
            let code = remote_with(&candidate, output);
            let assets = resolve_assets(argv);
            if code != 0 || assets.is_empty() {
                return code;
            }
            let backend = GhBackend {
                program: "gh".to_owned(),
                token: std::env::var("GH_TOKEN").ok(),
                api: release_api(
                    std::env::var("GH_REPO")
                        .ok()
                        .filter(|r| !r.is_empty())
                        .or_else(|| std::env::var("GITHUB_REPOSITORY").ok()),
                ),
            };
            let (text, code) = ledger_with(&backend, &tag, assets, approve == "1");
            print!("{text}");
            code
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
    fn release_api_defaults_and_follows_the_fork() {
        let default = "repos/devhusdk/rules_dx/releases/tags";
        assert_eq!(release_api(None), default);
        assert_eq!(release_api(Some(String::new())), default);
        assert_eq!(
            release_api(Some("me/rules_dx-forktest".to_owned())),
            "repos/me/rules_dx-forktest/releases/tags"
        );
    }

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

    #[test]
    fn assets_are_the_args_after_the_commit() {
        let argv: Vec<String> = ["d", "v1", SHA, "/o/a", "/o/b"].map(String::from).to_vec();
        assert_eq!(resolve_assets(&argv), ["/o/a", "/o/b"]);
        assert!(resolve_assets(&argv[..3]).is_empty());
        assert!(resolve_assets(&argv[..1]).is_empty());
    }

    #[cfg(unix)]
    fn fake_gh(dir: &Path, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::create_dir_all(dir).expect("dir");
        let script = dir.join("gh");
        std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        script.to_string_lossy().into_owned()
    }

    #[cfg(unix)]
    #[test]
    fn gh_failure_output_never_leaks_the_token() {
        let dir = std::env::temp_dir().join(format!("rd-leak-{}", std::process::id()));
        let asset = dir.join("a.bin");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(&asset, b"x").expect("asset");
        let backend = GhBackend {
            program: fake_gh(&dir.join("bin"), "echo \"token s3cret-token\" >&2; exit 1"),
            token: Some("s3cret-token".to_owned()),
            api: release_api(None),
        };
        let (text, code) = ledger_with(
            &backend,
            "v1.2.3",
            &[asset.to_string_lossy().into_owned()],
            true,
        );
        assert_eq!(code, 1);
        assert!(!text.contains("s3cret-token"));
        assert!(text.contains("***"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn matching_draft_release_validates_and_publish_stays_unauthorized() {
        let dir = std::env::temp_dir().join(format!("rd-ok-{}", std::process::id()));
        let asset = dir.join("a.bin");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(&asset, b"x").expect("asset");
        let sha = dx_digest::sha256_file_hex(&asset).expect("sha");
        let json = format!(
            "{{\"draft\":true,\"assets\":[{{\"name\":\"a.bin\",\"digest\":\"sha256:{sha}\"}}]}}"
        );
        let backend = GhBackend {
            program: fake_gh(&dir.join("bin"), &format!("echo '{json}'")),
            token: None,
            api: release_api(None),
        };
        let (text, code) = ledger_with(
            &backend,
            "v1.2.3",
            &[asset.to_string_lossy().into_owned()],
            true,
        );
        assert_eq!(code, 0);
        assert!(text.contains("\"state\": \"validated\""));
        assert!(text.contains("\"operation\": \"publish\""));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_asset_file_fails() {
        let (text, code) = ledger_with(
            &GhBackend {
                program: "gh".to_owned(),
                token: None,
                api: release_api(None),
            },
            "v1",
            &["/nonexistent/a.bin".to_owned()],
            true,
        );
        assert_eq!(code, 1);
        assert!(text.contains("asset-stage"));
    }
}
