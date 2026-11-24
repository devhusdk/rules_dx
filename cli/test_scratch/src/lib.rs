#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

use std::path::{Component, Path};
use std::process::Command;
pub use tempfile::TempDir;

pub mod wait;

const PREFIX_KEEP: usize = 24;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn bounded_prefix(prefix: &str) -> String {
    let sanitized: String = prefix
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' || char == '_' {
                char
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.len() <= PREFIX_KEEP {
        return sanitized;
    }
    let kept: String = sanitized.chars().take(PREFIX_KEEP).collect();
    format!("{kept}-{:x}", fnv1a(prefix.as_bytes()))
}

fn scratch_base(lookup: impl Fn(&str) -> Option<std::ffi::OsString>) -> std::path::PathBuf {
    lookup("TEST_TMPDIR")
        .filter(|dir| !dir.is_empty())
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
}

pub fn scratch(prefix: &str) -> Result<TempDir, String> {
    scratch_in(&scratch_base(|key| std::env::var_os(key)), prefix)
}

fn scratch_in(base: &Path, prefix: &str) -> Result<TempDir, String> {
    std::fs::create_dir_all(base).map_err(|err| format!("test scratch creates {base:?}: {err}"))?;
    let base = std::path::absolute(base)
        .map_err(|err| format!("test scratch resolves {base:?}: {err}"))?;
    tempfile::Builder::new()
        .prefix(&bounded_prefix(prefix))
        .tempdir_in(&base)
        .map_err(|err| format!("test scratch creates {prefix:?} in {base:?}: {err}"))
}

pub fn write(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    let stays_inside = Path::new(rel)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !stays_inside {
        return Err(format!("test scratch refuses {rel:?}: it leaves {root:?}"));
    }
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("test scratch creates {parent:?}: {err}"))?;
    }
    std::fs::write(&full, text).map_err(|err| format!("test scratch writes {full:?}: {err}"))
}

pub fn isolate_git<'a>(cmd: &'a mut Command, sandbox: &Path) -> &'a mut Command {
    let home = sandbox.join(".home");
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_CEILING_DIRECTORIES", sandbox);
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_AUTHOR_DATE",
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE",
        "GIT_NAMESPACE",
    ] {
        cmd.env_remove(var);
    }
    cmd
}

const GIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

pub fn git(root: &Path, sandbox: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root);
    isolate_git(&mut cmd, sandbox);
    cmd.args([
        "-c",
        "user.name=dx",
        "-c",
        "user.email=dx@example.com",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "tag.gpgsign=false",
    ])
    .args(args);
    let output = dx_process::capture::run_captured(&mut cmd, GIT_DEADLINE)
        .map_err(|err| format!("git {args:?} in {root:?}: {err}"))?;
    if !output.success() {
        let stderr = output.stderr_text();
        let detail = if stderr.is_empty() {
            output.stdout_text().trim().to_owned()
        } else {
            stderr
        };
        let status = output.code.map_or_else(
            || "killed by a signal".to_owned(),
            |code| format!("exit {code}"),
        );
        return Err(format!("git {args:?} failed ({status}): {detail}"));
    }
    Ok(output.stdout_text().trim().to_owned())
}

pub fn remove_directory_link(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let removed = std::fs::remove_dir(path);
    #[cfg(not(windows))]
    let removed = std::fs::remove_file(path);
    removed
}

pub fn eventually<T>(
    what: &str,
    deadline: std::time::Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> Result<T, String> {
    let start = std::time::Instant::now();
    loop {
        if let Some(value) = probe() {
            return Ok(value);
        }
        if start.elapsed() >= deadline {
            return Err(format!("timed out waiting for {what}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[must_use]
pub fn running_as_root() -> bool {
    #[cfg(unix)]
    let root = dx_os_sys::process::effective_user_id() == 0;
    #[cfg(not(unix))]
    let root = false;
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_in(repo: &Path) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(repo);
        cmd
    }

    #[cfg(windows)]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(windows))]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[test]
    fn git_reads_only_sandbox_config() {
        let sandbox = scratch("git-helper").unwrap();
        let root = sandbox.path().join("repo");
        std::fs::create_dir_all(&root).expect("dir");
        git(&root, sandbox.path(), &["init", "-q"]).unwrap();
        assert!(root.join(".git").is_dir());
        git(
            &root,
            sandbox.path(),
            &["commit", "-q", "--allow-empty", "-m", "x"],
        )
        .unwrap();
        assert_eq!(
            git(&root, sandbox.path(), &["log", "-1", "--format=%an <%ae>"]).unwrap(),
            "dx <dx@example.com>"
        );
        let origins = git(
            &root,
            sandbox.path(),
            &["config", "--list", "--show-origin"],
        )
        .unwrap();
        let sandbox_text = sandbox.path().to_string_lossy().into_owned();
        for line in origins.lines() {
            assert!(
                line.starts_with("command line:")
                    || line.starts_with("file:.git/")
                    || line.contains(&sandbox_text),
                "config outside the sandbox: {line}"
            );
        }
    }

    #[test]
    fn long_prefixes_stay_inside_the_windows_path_limit() {
        let long = "dx-exec-test-audit-live-exec__audit__audit_live__audit_live_license_names_uncovered_dependency_sets-license-__go_tests_fixtures_hello:hello-5-ws-";
        let bounded = bounded_prefix(long);
        assert!(bounded.len() <= PREFIX_KEEP + 1 + 16, "{}", bounded.len());
        assert!(bounded.starts_with("dx-exec-test-audit-l"), "{bounded}");
        assert_eq!(bounded, bounded_prefix(long), "stable");
        assert_ne!(bounded, bounded_prefix(&format!("{long}x")), "distinct");
        assert_eq!(
            bounded_prefix("dx-clean-walk-"),
            "dx-clean-walk-",
            "short prefixes pass through"
        );
        assert!(
            !bounded.contains(':'),
            "separators are sanitized: {bounded}"
        );
    }

    #[test]
    fn write_creates_nested_parents() {
        let dir = scratch("dx-test-scratch-write-").unwrap();
        write(dir.path(), "a/b/c.txt", "hello").unwrap();
        let text = std::fs::read_to_string(dir.path().join("a/b/c.txt")).expect("read back");
        assert_eq!(text, "hello");
    }

    #[test]
    fn write_overwrites_existing_file() {
        let dir = scratch("dx-test-scratch-write-").unwrap();
        write(dir.path(), "file.txt", "first").unwrap();
        write(dir.path(), "file.txt", "second").unwrap();
        let text = std::fs::read_to_string(dir.path().join("file.txt")).expect("read back");
        assert_eq!(text, "second");
    }

    #[test]
    fn scratch_uses_prefix_and_exists() {
        let dir = scratch("dx-test-scratch-").unwrap();
        assert!(dir.path().is_dir(), "scratch exists: {:?}", dir.path());
        let name = dir.path().file_name().expect("tempdir has a name");
        assert!(
            name.to_string_lossy().starts_with("dx-test-scratch-"),
            "prefix honored: {name:?}"
        );
    }

    #[test]
    fn scratch_base_prefers_test_tmpdir() {
        let set = scratch_base(|_| Some("/bazel/test_tmp".into()));
        assert_eq!(set, std::path::PathBuf::from("/bazel/test_tmp"));
        assert_eq!(scratch_base(|_| Some("".into())), std::env::temp_dir());
        assert_eq!(scratch_base(|_| None), std::env::temp_dir());
    }

    #[test]
    fn scratch_lives_under_test_tmpdir_under_bazel() {
        let Some(base) = std::env::var_os("TEST_TMPDIR").filter(|dir| !dir.is_empty()) else {
            return;
        };
        let dir = scratch("dx-test-scratch-base-").unwrap();
        assert!(
            dir.path().starts_with(&base),
            "{:?} under {base:?}",
            dir.path()
        );
    }

    #[test]
    fn scratches_are_unique() {
        let first = scratch("dx-test-scratch-").unwrap();
        let second = scratch("dx-test-scratch-").unwrap();
        assert_ne!(first.path(), second.path(), "unique dirs");
    }

    #[test]
    fn scratch_cleans_on_drop() {
        let path = {
            let dir = scratch("dx-test-scratch-").unwrap();
            let path = dir.path().to_path_buf();
            assert!(path.is_dir(), "exists before drop");
            path
        };
        assert!(!path.exists(), "removed on drop: {path:?}");
    }

    #[test]
    fn remove_directory_link_keeps_the_target() {
        let dir = scratch("dx-test-scratch-link-").unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join("marker"), "marker").expect("marker");
        let link = dir.path().join("link");
        directory_link(&target, &link).expect("directory link");
        remove_directory_link(&link).expect("remove directory link");
        assert!(std::fs::symlink_metadata(&link).is_err(), "link removed");
        assert_eq!(
            std::fs::read_to_string(target.join("marker")).expect("marker survives"),
            "marker",
            "the target is not followed"
        );
    }

    #[test]
    fn isolate_git_ignores_ambient_repos_and_config() {
        let dir = scratch("dx-isolate-git-").unwrap();
        let sandbox = dir.path().canonicalize().expect("canonical");
        let repo = sandbox.join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        let mut probe = git_in(&repo);
        probe.args(["rev-parse", "--is-inside-work-tree"]);
        isolate_git(&mut probe, &sandbox);
        assert!(!probe.output().expect("git").status.success(), "not a repo");
        let mut init = git_in(&repo);
        init.args(["init", "-q"]);
        isolate_git(&mut init, &sandbox);
        assert!(init.status().expect("git").success());
        let mut list = git_in(&repo);
        list.args(["config", "--global", "--list"]);
        isolate_git(&mut list, &sandbox);
        let out = list.output().expect("git");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
    }

    #[test]
    fn eventually_returns_the_first_some() {
        let mut calls = 0;
        let got = eventually("x", std::time::Duration::from_secs(5), || {
            calls += 1;
            Some(7)
        })
        .unwrap();
        assert_eq!((got, calls), (7, 1));
    }

    #[test]
    fn eventually_retries_until_the_probe_succeeds() {
        let mut calls = 0;
        let got = eventually("x", std::time::Duration::from_secs(5), || {
            calls += 1;
            (calls == 3).then_some(calls)
        })
        .unwrap();
        assert_eq!(got, 3);
    }

    #[cfg(unix)]
    #[test]
    fn running_as_root_matches_reading_a_mode_000_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("dx-test-scratch-root-").unwrap();
        let locked = dir.path().join("locked");
        std::fs::write(&locked, "x").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(running_as_root(), std::fs::read(&locked).is_ok());
    }

    #[test]
    fn eventually_fails_after_the_deadline() {
        assert_eq!(
            eventually::<()>("a thing", std::time::Duration::from_millis(50), || None),
            Err("timed out waiting for a thing".to_owned())
        );
    }

    #[test]
    fn write_refuses_paths_that_leave_the_root() {
        let dir = scratch("dx-test-scratch-escape-").unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("outside.txt");
        let absolute = outside.to_string_lossy().into_owned();
        for rel in [
            absolute.as_str(),
            "../outside.txt",
            "a/../../outside.txt",
            "C:\\x\\y",
        ] {
            let result = write(&root, rel, "x");
            if cfg!(windows) || rel != "C:\\x\\y" {
                assert!(result.is_err(), "{rel} must be refused");
            }
        }
        assert!(!outside.exists(), "nothing written outside the root");
    }

    #[test]
    fn git_error_names_stdout_when_stderr_is_empty() {
        let sandbox = scratch("git-empty-stderr").unwrap();
        let root = sandbox.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, sandbox.path(), &["init", "-q"]).unwrap();
        git(
            &root,
            sandbox.path(),
            &["commit", "-q", "--allow-empty", "-m", "x"],
        )
        .unwrap();
        let err = git(&root, sandbox.path(), &["commit", "-m", "again"]).unwrap_err();
        assert!(err.contains("nothing to commit"), "{err}");
        assert!(err.contains("exit"), "{err}");
    }

    #[test]
    fn isolate_git_drops_identity_and_date_overrides() {
        let sandbox = scratch("git-identity").unwrap();
        let root = sandbox.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, sandbox.path(), &["init", "-q"]).unwrap();
        let mut commit = git_in(&root);
        commit.args([
            "-c",
            "user.name=dx",
            "-c",
            "user.email=dx@example.com",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ]);
        for var in [
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
        ] {
            commit.env(var, "ambient");
        }
        isolate_git(&mut commit, sandbox.path());
        assert!(commit.status().unwrap().success());
        assert_eq!(
            git(
                &root,
                sandbox.path(),
                &["log", "-1", "--format=%an <%ae> %cn <%ce>"]
            )
            .unwrap(),
            "dx <dx@example.com> dx <dx@example.com>"
        );
    }

    #[test]
    fn scratch_creates_a_missing_base_and_returns_absolute_paths() {
        let dir = scratch("dx-test-scratch-base-").unwrap();
        let missing = dir.path().join("not/yet");
        let made = scratch_in(&missing, "dx-x-").unwrap();
        assert!(made.path().is_dir());
        assert!(made.path().is_absolute());
        let relative = scratch_in(Path::new("."), "dx-rel-").unwrap();
        assert!(relative.path().is_absolute(), "{:?}", relative.path());
    }

    const HOLD: &str = "DX_TEST_SCRATCH_HOLD";

    #[test]
    fn holds_until_stdin_closes() {
        if std::env::var_os(HOLD).is_some() {
            drop(std::io::Read::read_to_end(
                &mut std::io::stdin(),
                &mut Vec::new(),
            ));
        }
    }

    fn holder() -> std::process::Child {
        Command::new(std::env::current_exe().expect("exe"))
            .args(["--exact", "tests::holds_until_stdin_closes"])
            .env(HOLD, "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawn")
    }

    const LONG: std::time::Duration = std::time::Duration::from_secs(60);

    #[test]
    fn within_returns_the_work_or_names_what_never_finished() {
        assert_eq!(wait::within("one", LONG, || 1), Ok(1));
        let (_keep, never) = std::sync::mpsc::channel::<()>();
        let error = wait::within("the never", std::time::Duration::ZERO, move || never.recv())
            .expect_err("never finishes");
        assert!(error.contains("the never did not happen"), "{error}");
    }

    #[test]
    fn path_appears_sees_a_file_created_below_a_missing_directory() {
        let dir = scratch("dx-wait-path-").unwrap();
        let path = dir.path().join("later/deeper/file");
        let error = wait::path_appears(&path, std::time::Duration::ZERO).expect_err("absent");
        assert!(error.contains("appearing did not happen"), "{error}");
        let root = dir.path().to_path_buf();
        let writer = std::thread::spawn(move || write(&root, "later/deeper/file", "x"));
        wait::path_appears(&path, LONG).unwrap();
        writer.join().expect("writer").unwrap();
    }

    #[test]
    fn lock_free_waits_for_the_holder_to_let_go() {
        let dir = scratch("dx-wait-lock-").unwrap();
        let path = dir.path().join("alive.lock");
        let held = std::fs::File::create(&path).unwrap();
        held.lock().unwrap();
        let error = wait::lock_free(&path, std::time::Duration::ZERO).expect_err("held");
        assert!(error.contains("coming free did not happen"), "{error}");
        drop(held);
        wait::lock_free(&path, LONG).unwrap();
    }

    #[test]
    fn pid_gone_and_exit_within_follow_a_child_to_its_end() {
        let mut child = holder();
        let pid = child.id();
        assert!(wait::pid_gone(pid, std::time::Duration::ZERO).is_err());
        drop(child.stdin.take());
        wait::pid_gone(pid, LONG).unwrap();
        assert!(matches!(
            wait::exit_within(child, LONG).unwrap(),
            wait::Exit::Exited(status) if status.success()
        ));
    }

    #[test]
    fn exit_within_kills_a_child_that_outlives_the_limit() {
        let mut child = holder();
        let _stdin = child.stdin.take();
        assert!(matches!(
            wait::exit_within(child, std::time::Duration::ZERO).unwrap(),
            wait::Exit::Killed(status) if !status.success()
        ));
    }
}
