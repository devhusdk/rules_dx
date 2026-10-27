#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::Path;
use std::process::Command;
pub use tempfile::TempDir;

/// Longest prefix kept verbatim; the rest is folded into a checksum.
const PREFIX_KEEP: usize = 24;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Keeps the temp dir name short enough for the Windows 260-char path limit.
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
    format!(
        "{}-{:x}",
        &sanitized[..PREFIX_KEEP],
        fnv1a(prefix.as_bytes())
    )
}

/// Picks where scratch dirs live: `$TEST_TMPDIR` when set and not empty, so
/// Bazel wipes leftovers of killed tests, else the system temp dir.
fn scratch_base(lookup: impl Fn(&str) -> Option<std::ffi::OsString>) -> std::path::PathBuf {
    lookup("TEST_TMPDIR")
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Creates a temp dir under `$TEST_TMPDIR` (or the system temp dir) that is
/// removed when the returned guard drops. Keep the guard bound for as long as
/// anything, including child processes, uses paths inside it.
pub fn scratch(prefix: &str) -> TempDir {
    let bounded = bounded_prefix(prefix);
    let base = scratch_base(|key| std::env::var_os(key));
    tempfile::Builder::new()
        .prefix(&bounded)
        .tempdir_in(&base)
        .unwrap_or_else(|err| panic!("test scratch creates {prefix:?}: {err}"))
}

/// Writes `text` to `root/rel`, creating missing parent directories.
pub fn write(root: &Path, rel: &str, text: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("test scratch creates {parent:?}: {err}"));
    }
    std::fs::write(&full, text).unwrap_or_else(|err| panic!("test scratch writes {full:?}: {err}"));
}

/// Makes a git command hermetic: it ignores system and user config, and cannot
/// discover a repository above `sandbox`.
///
/// `XDG_CONFIG_HOME` is redirected too because git reads `core.excludesFile`
/// from `$XDG_CONFIG_HOME/git/ignore` even when `GIT_CONFIG_GLOBAL` is set.
/// `GIT_CEILING_DIRECTORIES` stops discovery from walking out of the sandbox.
/// Repository-pointing variables are removed so tests run from a git hook do
/// not write to the developer's real repository. Writes nothing to disk.
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
    ] {
        cmd.env_remove(var);
    }
    cmd
}

/// Removes a directory link without following it into its target.
pub fn remove_directory_link(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let removed = std::fs::remove_dir(path);
    #[cfg(not(windows))]
    let removed = std::fs::remove_file(path);
    removed
}

/// Symlinks host tools into `dir` so a test can run a script with an exact PATH.
///
/// Each name is looked up on the test process PATH, falling back to `/usr/bin` and `/bin`.
/// Links are symlinks, not copies, because coreutils and busybox dispatch on argv[0].
/// Returns the names that were not found.
#[cfg(unix)]
pub fn link_host_tools(dir: &Path, names: &[&str]) -> std::io::Result<Vec<String>> {
    let mut search: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    search.push("/usr/bin".into());
    search.push("/bin".into());
    std::fs::create_dir_all(dir)?;
    let mut missing = Vec::new();
    for name in names {
        let found = search.iter().map(|d| d.join(name)).find(|c| c.is_file());
        match found {
            Some(target) => {
                let link = dir.join(name);
                if !link.exists() {
                    std::os::unix::fs::symlink(&target, &link)?;
                }
            }
            None => missing.push((*name).to_owned()),
        }
    }
    Ok(missing)
}

/// Retries `probe` every 25ms until it returns `Some`, or panics once
/// `deadline` has passed.
pub fn eventually<T>(
    what: &str,
    deadline: std::time::Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> T {
    let start = std::time::Instant::now();
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(start.elapsed() < deadline, "timed out waiting for {what}");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(windows))]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
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

    #[cfg(unix)]
    #[test]
    fn host_tools_are_linked_and_missing_ones_reported() {
        let dir = scratch("dx-test-scratch-tools-");
        let tools = dir.path().join("tools");
        let missing = link_host_tools(&tools, &["sh", "dx-no-such-tool-xyz"]).expect("links");
        assert_eq!(missing, vec!["dx-no-such-tool-xyz"]);
        assert!(tools.join("sh").exists());
        assert!(!tools.join("dx-no-such-tool-xyz").exists());
    }

    #[test]
    fn write_creates_nested_parents() {
        let dir = scratch("dx-test-scratch-write-");
        write(dir.path(), "a/b/c.txt", "hello");
        let text = std::fs::read_to_string(dir.path().join("a/b/c.txt")).expect("read back");
        assert_eq!(text, "hello");
    }

    #[test]
    fn write_overwrites_existing_file() {
        let dir = scratch("dx-test-scratch-write-");
        write(dir.path(), "file.txt", "first");
        write(dir.path(), "file.txt", "second");
        let text = std::fs::read_to_string(dir.path().join("file.txt")).expect("read back");
        assert_eq!(text, "second");
    }

    #[test]
    fn scratch_uses_prefix_and_exists() {
        let dir = scratch("dx-test-scratch-");
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
        let dir = scratch("dx-test-scratch-base-");
        assert!(
            dir.path().starts_with(&base),
            "{:?} under {base:?}",
            dir.path()
        );
    }

    #[test]
    fn scratches_are_unique() {
        let first = scratch("dx-test-scratch-");
        let second = scratch("dx-test-scratch-");
        assert_ne!(first.path(), second.path(), "unique dirs");
    }

    #[test]
    fn scratch_cleans_on_drop() {
        let path = {
            let dir = scratch("dx-test-scratch-");
            let path = dir.path().to_path_buf();
            assert!(path.is_dir(), "exists before drop");
            path
        };
        assert!(!path.exists(), "removed on drop: {path:?}");
    }

    #[test]
    fn remove_directory_link_keeps_the_target() {
        let dir = scratch("dx-test-scratch-link-");
        let target = dir.path().join("target");
        std::fs::create_dir(&target).expect("target");
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
        let dir = scratch("dx-isolate-git-");
        let sandbox = dir.path().canonicalize().expect("canonical");
        let repo = sandbox.join("repo");
        std::fs::create_dir(&repo).expect("repo dir");
        let mut probe = Command::new("git");
        probe
            .arg("-C")
            .arg(&repo)
            .args(["rev-parse", "--is-inside-work-tree"]);
        isolate_git(&mut probe, &sandbox);
        assert!(!probe.output().expect("git").status.success(), "not a repo");
        let mut init = Command::new("git");
        init.arg("-C").arg(&repo).args(["init", "-q"]);
        isolate_git(&mut init, &sandbox);
        assert!(init.status().expect("git").success());
        let mut list = Command::new("git");
        list.arg("-C")
            .arg(&repo)
            .args(["config", "--global", "--list"]);
        isolate_git(&mut list, &sandbox);
        let out = list.output().expect("git");
        assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());
    }

    #[test]
    fn eventually_returns_the_first_some() {
        let mut calls = 0;
        let got = eventually("x", std::time::Duration::from_secs(5), || {
            calls += 1;
            Some(7)
        });
        assert_eq!((got, calls), (7, 1));
    }

    #[test]
    fn eventually_retries_until_the_probe_succeeds() {
        let mut calls = 0;
        let got = eventually("x", std::time::Duration::from_secs(5), || {
            calls += 1;
            (calls == 3).then_some(calls)
        });
        assert_eq!(got, 3);
    }

    #[test]
    #[should_panic(expected = "timed out waiting for a thing")]
    fn eventually_panics_after_the_deadline() {
        eventually::<()>("a thing", std::time::Duration::from_millis(50), || None);
    }
}
