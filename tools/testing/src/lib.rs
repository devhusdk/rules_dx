#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};

pub mod cli;
pub mod fatbin;
pub mod latency;

pub use dx_binfmt::machine as image_machine;

pub use serde_json;

pub struct Run {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn combined(&self) -> String {
        let mut out = self.stdout.clone();
        out.push_str(&self.stderr);
        out
    }
}

/// Runs one tool on the runfiles built beside it, never on the runfiles of this process.
///
/// Returns a `Command` for `bin` with `envs` set and the runfiles variables pointing at `bin`'s own
/// runfiles, so callers can add arguments and choose how to collect output.
pub fn command_for(bin: &Path, envs: &[(&str, &str)]) -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(bin);
    command.envs(envs.iter().copied());
    match dx_path::Resolver::for_binary(bin) {
        Ok(own) => {
            let source = own.source().to_path_buf();
            if source.is_dir() {
                command.env_remove("RUNFILES_MANIFEST_FILE");
                command.env("RUNFILES_DIR", source);
            } else {
                command.env("RUNFILES_MANIFEST_FILE", source);
                command.env_remove("RUNFILES_DIR");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(command)
}

pub fn run(bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::io::Result<Run> {
    let output = command_for(bin, envs)?.args(args).output()?;
    Ok(Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

static SCRATCH_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn mkscratch(prefix: &str) -> std::io::Result<PathBuf> {
    let base = std::env::var("TEST_TMPDIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let seq = SCRATCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = base.join(format!("{prefix}-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

pub fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    text.push('\n');
    std::fs::write(path, text.as_bytes())
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn read_lines(path: &Path) -> std::io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.split('\n').map(str::to_owned).collect())
}

const POWERSHELL_SUFFIXES: [&str; 1] = [".ps1"];

fn suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        &POWERSHELL_SUFFIXES
    } else {
        &[]
    }
}

fn suffixed(key: &str) -> Vec<String> {
    if Path::new(key).extension().is_some() {
        return Vec::new();
    }
    dx_path::host::EXECUTABLE_SUFFIXES
        .iter()
        .copied()
        .chain(suffixes().iter().copied())
        .map(|suffix| format!("{key}{suffix}"))
        .collect()
}

fn workspace() -> String {
    std::env::var("TEST_WORKSPACE")
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "_main".to_owned())
}

/// Returns the runfiles key that one `$(rootpath)` or `$(rlocationpath)` spells.
pub fn rlocation_key(rel: &str) -> String {
    if let Some(external) = rel.strip_prefix("../") {
        return external.to_owned();
    }
    let workspace = workspace();
    if rel
        .strip_prefix(workspace.as_str())
        .is_some_and(|rest| rest.starts_with('/'))
    {
        return rel.to_owned();
    }
    format!("{workspace}/{rel}")
}

fn runfiles_source() -> String {
    for name in ["RUNFILES_MANIFEST_FILE", "RUNFILES_DIR", "TEST_SRCDIR"] {
        if let Some(value) = std::env::var_os(name).filter(|value| !value.is_empty()) {
            return format!("{name}={}", Path::new(&value).display());
        }
    }
    "the runfiles beside argv[0]".to_owned()
}

fn runfiles_tree() -> Option<PathBuf> {
    ["RUNFILES_DIR", "TEST_SRCDIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .find(|dir| dir.is_dir())
}

fn existing(runfiles: &runfiles::Runfiles, key: &str) -> Option<PathBuf> {
    runfiles
        .rlocation_from(key, "")
        .filter(|path| path.exists())
}

/// Resolves one runfiles key through the Bazel runfiles of this process.
pub fn rlocation(key: &str) -> std::io::Result<PathBuf> {
    let opened = runfiles::Runfiles::create();
    if let Some(found) = opened
        .as_ref()
        .ok()
        .and_then(|runfiles| existing(runfiles, key))
    {
        return Ok(found);
    }
    if let Some(tree) = runfiles_tree() {
        let directory = runfiles::Runfiles::builder().directory(&tree).build();
        if let Some(found) = directory.ok().and_then(|runfiles| existing(&runfiles, key)) {
            return Ok(found);
        }
    }
    let detail = match opened {
        Ok(_) => format!("{key} is not in {}", runfiles_source()),
        Err(error) => format!("cannot open {} for {key}: {error}", runfiles_source()),
    };
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, detail))
}

/// Resolves one `$(rootpath)` or `$(rlocationpath)` test input, panicking with its key when absent.
pub fn resolve_runfiles(rel: &str) -> PathBuf {
    find_runfile(rel).unwrap_or_else(|error| panic!("runfile {rel}: {error}"))
}

fn find_runfile(rel: &str) -> std::io::Result<PathBuf> {
    let direct = Path::new(rel);
    if direct.is_absolute() {
        return Ok(direct.to_path_buf());
    }
    let key = rlocation_key(rel);
    let error = match rlocation(&key) {
        Ok(found) => return Ok(found),
        Err(error) => error,
    };
    suffixed(&key)
        .iter()
        .find_map(|candidate| rlocation(candidate).ok())
        .ok_or(error)
}

/// Names the rules_dx checkout behind the `//:MODULE.bazel` runfile of the calling test.
pub fn rules_dx_source() -> PathBuf {
    let module = find_runfile("MODULE.bazel").unwrap_or_else(|error| {
        panic!(
            "runfile MODULE.bazel: {error}; add //:module_srcs or an area module_srcs to the test's data"
        )
    });
    let real = std::fs::canonicalize(&module).unwrap_or_else(|error| panic!("{module:?}: {error}"));
    real.parent()
        .unwrap_or_else(|| panic!("no checkout above {real:?}"))
        .to_path_buf()
}

/// Writes `text` to `path`, creating its parent directories, and panics with the path on failure.
pub fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

/// Reads `path` as text and panics with the path on failure.
pub fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

/// Names the Bazel-built process probe from the runfiles of the calling test.
pub fn process_probe() -> PathBuf {
    let rel = std::env::var("DX_PROCESS_PROBE")
        .unwrap_or_else(|_| panic!("DX_PROCESS_PROBE must name the process probe"));
    resolve_runfiles(&rel)
}

/// Reports whether a live process still holds the alive lock at `lock`.
pub fn alive_lock_held(lock: &Path) -> bool {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock)
        .unwrap_or_else(|error| panic!("open {}: {error}", lock.display()));
    match file.try_lock() {
        Ok(()) => false,
        Err(std::fs::TryLockError::WouldBlock) => true,
        Err(std::fs::TryLockError::Error(error)) => {
            panic!("lock probe {}: {error}", lock.display())
        }
    }
}

/// Waits up to 10s for the process holding the alive lock at `lock` to die.
pub fn assert_alive_lock_released(lock: &Path) {
    assert!(lock.exists(), "the probe never armed: {}", lock.display());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while alive_lock_held(lock) {
        assert!(
            std::time::Instant::now() < deadline,
            "an owned process survived: {}",
            lock.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn runfiles_root() -> PathBuf {
    let root =
        std::env::var("TEST_SRCDIR").unwrap_or_else(|_| panic!("TEST_SRCDIR is set under Bazel"));
    let workspace = std::env::var("TEST_WORKSPACE")
        .unwrap_or_else(|_| panic!("TEST_WORKSPACE is set under Bazel"));
    Path::new(&root).join(workspace)
}

pub fn read_runfiles(rel: &str) -> String {
    let path = resolve_runfiles(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {rel}: {error}"))
}

pub fn starlark_const(text: &str, name: &str, file: &str) -> String {
    let prefix = format!("{name} = \"");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{file} has no {name}"))
        .trim_end_matches('"')
        .to_owned()
}

/// Returns the workflow runfiles paths that DX_WORKFLOWS lists.
pub fn workflow_files() -> Vec<String> {
    let list = std::env::var("DX_WORKFLOWS").unwrap_or_else(|_| {
        panic!(
            "DX_WORKFLOWS is unset; add `env = {{\"DX_WORKFLOWS\": \"$(rootpaths //.github:workflows)\"}}` and `//.github:workflows` to this test's data"
        )
    });
    workflow_paths(&list, "DX_WORKFLOWS")
}

/// Parses a whitespace-separated workflow list into sorted, unique paths.
pub fn workflow_paths(list: &str, source: &str) -> Vec<String> {
    let mut paths: Vec<String> = list.split_whitespace().map(str::to_owned).collect();
    paths.sort();
    paths.dedup();
    assert!(!paths.is_empty(), "{source} names no workflow");
    for path in &paths {
        assert!(
            path.starts_with(".github/workflows/")
                && (path.ends_with(".yml") || path.ends_with(".yaml")),
            "{source} entry {path} is not a .github/workflows/*.yml or *.yaml file"
        );
    }
    paths
}

pub fn expect_contains(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut missing = Vec::new();
    for pat in patterns {
        if !lines.iter().any(|line| line.contains(pat)) {
            missing.push(format!("[{pat}]"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} missing literals:{}",
            path.display(),
            missing.join(" ")
        ))
    }
}

pub fn expect_absent(path: &Path, patterns: &[&str]) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing file {}", path.display()));
    }
    let lines = read_lines(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut present = Vec::new();
    for pat in patterns {
        if lines.iter().any(|line| line.contains(pat)) {
            present.push(format!("[{pat}]"));
        }
    }
    if present.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} must not contain:{}",
            path.display(),
            present.join(" ")
        ))
    }
}

fn workspace_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if text.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(text))
                }
            } else {
                None
            }
        })
}

fn update_expect() -> bool {
    std::env::var("UPDATE_EXPECT").as_deref() == Ok("1")
}

fn stage_update(actual: &Path, label: &str) -> Option<PathBuf> {
    let out_dir = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("TMPDIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    std::fs::create_dir_all(&out_dir).ok()?;
    let staged = out_dir.join(format!("{label}.expected.update"));
    std::fs::copy(actual, &staged).ok()?;
    Some(staged)
}

fn unified_diff(expected: &str, actual: &str) -> String {
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let mut out = vec!["--- expected".to_owned(), "+++ actual".to_owned()];
    let max = old.len().max(new.len());
    for i in 0..max {
        let old_line = old.get(i).copied().unwrap_or("");
        let new_line = new.get(i).copied().unwrap_or("");
        if old_line != new_line {
            if i < old.len() {
                out.push(format!("-{old_line}"));
            }
            if i < new.len() {
                out.push(format!("+{new_line}"));
            }
        }
    }
    out.join("\n")
}

pub fn snapshot_diff(
    expected: &Path,
    actual: &Path,
    workspace_rel: Option<&str>,
) -> Result<(), String> {
    let expected_bytes = std::fs::read(expected)
        .map_err(|err| format!("cannot read {}: {err}", expected.display()))?;
    let actual_bytes =
        std::fs::read(actual).map_err(|err| format!("cannot read {}: {err}", actual.display()))?;
    if expected_bytes == actual_bytes {
        return Ok(());
    }
    if update_expect() {
        if let Some(rel) = workspace_rel {
            if let Some(root) = workspace_root() {
                if root.is_dir() {
                    let dest = root.join(rel);
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::copy(actual, &dest)
                        .map_err(|err| format!("cannot refresh {}: {err}", dest.display()))?;
                    return Ok(());
                }
            }
        }
        let expected_text = expected.to_string_lossy();
        if !expected_text.contains("/runfiles/") && std::fs::copy(actual, expected).is_ok() {
            return Ok(());
        }
        let label = expected
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("expected");
        if let Some(staged) = stage_update(actual, label) {
            let mut hint = format!(
                "snapshot UPDATE_EXPECT: staged refreshed golden at {}",
                staged.display()
            );
            if let Some(rel) = workspace_rel {
                hint.push_str(&format!("; copy it to {rel} in the checkout, then re-run"));
            }
            return Err(hint);
        }
        return Err("UPDATE_EXPECT staged refresh failed".to_owned());
    }
    let expected_text = String::from_utf8_lossy(&expected_bytes);
    let actual_text = String::from_utf8_lossy(&actual_bytes);
    let diff = unified_diff(&expected_text, &actual_text);
    Err(format!(
        "snapshot FAIL: {} differs from actual\n{diff}\nre-run with UPDATE_EXPECT=1 to refresh the golden, then review the diff before committing",
        expected.display()
    ))
}

pub fn assert_valid_json(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map(|_| ())
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("scratch tempdir")
    }

    fn write(dir: &Path, rel: &str, text: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parents");
        }
        let mut handle = std::fs::File::create(&path).expect("create");
        handle.write_all(text.as_bytes()).expect("write");
        path
    }

    #[test]
    fn write_text_creates_parents_and_read_text_reads_it_back() {
        let dir = scratch("dx-testing-text-");
        let path = dir.path().join("a/b/c.txt");
        write_text(&path, "hello\n");
        assert_eq!(read_text(&path), "hello\n");
    }

    #[test]
    fn rules_dx_source_holds_the_module_and_its_bazel_version() {
        let source = rules_dx_source();
        assert!(source.join("MODULE.bazel").is_file(), "{source:?}");
        assert!(source.join(".bazelversion").is_file(), "{source:?}");
    }

    #[test]
    fn runfiles_resolution_prefers_an_existing_file() {
        let dir = scratch("dx-testing-runfiles-");
        let direct = write(dir.path(), "pkg/tool", "#!/bin/sh\n");
        assert_eq!(resolve_runfiles(&direct.to_string_lossy()), direct);
    }

    fn child_dir() -> Option<PathBuf> {
        std::env::var_os("DX_TESTING_CHILD").map(PathBuf::from)
    }

    fn in_child(name: &str, dir: &Path, envs: &[(&str, &Path)]) {
        let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
        command
            .args([name, "--exact", "--test-threads=1"])
            .env("DX_TESTING_CHILD", dir);
        for key in [
            "RUNFILES_MANIFEST_FILE",
            "RUNFILES_DIR",
            "TEST_SRCDIR",
            "BUILD_WORKSPACE_DIRECTORY",
        ] {
            command.env_remove(key);
        }
        for (key, value) in envs {
            command.env(key, value);
        }
        let output = command.output().expect("spawn child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{name} failed in its child: {stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains("1 passed"), "{name} did not run: {stdout}");
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read resolved runfile")
    }

    fn panic_text(rel: &str) -> String {
        let payload = std::panic::catch_unwind(|| resolve_runfiles(rel)).expect_err("panics");
        payload
            .downcast_ref::<String>()
            .cloned()
            .expect("panic message")
    }

    #[test]
    fn rootpaths_and_rlocationpaths_name_one_key() {
        assert_eq!(rlocation_key("pkg/tool"), "_main/pkg/tool");
        assert_eq!(rlocation_key("_main/pkg/tool"), "_main/pkg/tool");
        assert_eq!(rlocation_key("../dep+/pkg/tool"), "dep+/pkg/tool");
        assert_eq!(rlocation_key("_mainline/tool"), "_main/_mainline/tool");
    }

    #[test]
    fn a_manifest_resolves_escaped_keys() {
        if let Some(dir) = child_dir() {
            assert_eq!(read(&resolve_runfiles("pkg/a b.txt")), "spaced\n");
            assert_eq!(read(&resolve_runfiles("pkg/c\\d\ne")), "escaped\n");
            assert_eq!(read(&resolve_runfiles("_main/pkg/plain.txt")), "plain\n");
            let raw = rlocation("_main/pkg/a\\sb.txt").expect_err("an escape is not a key");
            assert!(
                raw.to_string()
                    .contains(&dir.join("MANIFEST").display().to_string()),
                "{raw}"
            );
            return;
        }
        let dir = scratch("dx-testing-escaped-");
        let spaced = write(dir.path(), "real dir/a b.txt", "spaced\n");
        let escaped = write(dir.path(), "escaped.txt", "escaped\n");
        let plain = write(dir.path(), "plain.txt", "plain\n");
        let manifest = write(
            dir.path(),
            "MANIFEST",
            &format!(
                " _main/pkg/a\\sb.txt {}\n _main/pkg/c\\bd\\ne {}\n_main/pkg/plain.txt {}\n",
                spaced.display(),
                escaped.display(),
                plain.display()
            ),
        );
        in_child(
            "tests::a_manifest_resolves_escaped_keys",
            dir.path(),
            &[("RUNFILES_MANIFEST_FILE", &manifest)],
        );
    }

    #[test]
    fn an_update_without_a_checkout_fails_and_names_the_staged_file() {
        if let Some(dir) = child_dir() {
            let golden = dir.join("runfiles/_main/pkg/golden.txt");
            let actual = dir.join("actual.txt");
            let err = snapshot_diff(&golden, &actual, Some("pkg/golden.txt"))
                .expect_err("a staged-only refresh must fail");
            let staged = dir.join("out/golden.txt.expected.update");
            assert!(err.contains(&staged.display().to_string()), "{err}");
            assert!(err.contains("pkg/golden.txt"), "{err}");
            assert_eq!(read(&staged), "new\n");
            assert_eq!(read(&golden), "old\n");
            return;
        }
        let dir = scratch("dx-testing-update-");
        let golden = write(dir.path(), "runfiles/_main/pkg/golden.txt", "old\n");
        write(dir.path(), "actual.txt", "new\n");
        let mut perms = std::fs::metadata(&golden).expect("meta").permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&golden, perms).expect("read-only golden");
        let out = dir.path().join("out");
        let nogit = dir.path().join("nogit");
        in_child(
            "tests::an_update_without_a_checkout_fails_and_names_the_staged_file",
            dir.path(),
            &[
                ("UPDATE_EXPECT", Path::new("1")),
                ("TEST_UNDECLARED_OUTPUTS_DIR", &out),
                ("GIT_DIR", &nogit),
            ],
        );
    }

    #[test]
    fn a_tree_resolves_files_directories_and_external_repositories() {
        if let Some(dir) = child_dir() {
            let tree = dir.join("tree");
            assert_eq!(
                resolve_runfiles("pkg/tool.txt"),
                tree.join("_main/pkg/tool.txt")
            );
            assert_eq!(resolve_runfiles("pkg"), tree.join("_main/pkg"));
            assert_eq!(read(&resolve_runfiles("../dep+/data.txt")), "external\n");
            return;
        }
        let dir = scratch("dx-testing-tree-");
        write(dir.path(), "tree/_main/pkg/tool.txt", "tool\n");
        write(dir.path(), "tree/dep+/data.txt", "external\n");
        in_child(
            "tests::a_tree_resolves_files_directories_and_external_repositories",
            dir.path(),
            &[("RUNFILES_DIR", &dir.path().join("tree"))],
        );
    }

    #[test]
    fn an_apparent_repository_name_resolves_through_the_repo_mapping() {
        if child_dir().is_some() {
            assert_eq!(
                read(&rlocation("dep/pkg/data.txt").expect("mapped")),
                "dep\n"
            );
            assert_eq!(
                read(&resolve_runfiles("../dep+1.0.0+/pkg/data.txt")),
                "dep\n"
            );
            assert!(rlocation("other/pkg/data.txt").is_err());
            return;
        }
        let dir = scratch("dx-testing-mapping-");
        let data = write(dir.path(), "data.txt", "dep\n");
        let mapping = write(dir.path(), "repo_mapping", ",dep,dep+1.0.0+\n");
        let manifest = write(
            dir.path(),
            "MANIFEST",
            &format!(
                "_repo_mapping {}\ndep+1.0.0+/pkg/data.txt {}\n",
                mapping.display(),
                data.display()
            ),
        );
        in_child(
            "tests::an_apparent_repository_name_resolves_through_the_repo_mapping",
            dir.path(),
            &[("RUNFILES_MANIFEST_FILE", &manifest)],
        );
    }

    #[test]
    fn a_launcher_suffix_is_tried_after_the_exact_key() {
        if child_dir().is_some() {
            assert_eq!(read(&resolve_runfiles("pkg/tool")), "launcher\n");
            assert_eq!(read(&resolve_runfiles("pkg/both")), "exact\n");
            return;
        }
        let dir = scratch("dx-testing-suffix-");
        let launcher = write(dir.path(), "tool.bat", "launcher\n");
        let exact = write(dir.path(), "both", "exact\n");
        let manifest = write(
            dir.path(),
            "MANIFEST",
            &format!(
                "_main/pkg/tool.bat {}\n_main/pkg/both.exe {}\n_main/pkg/both {}\n",
                launcher.display(),
                launcher.display(),
                exact.display()
            ),
        );
        in_child(
            "tests::a_launcher_suffix_is_tried_after_the_exact_key",
            dir.path(),
            &[("RUNFILES_MANIFEST_FILE", &manifest)],
        );
    }

    #[test]
    fn a_missing_key_names_the_input_the_key_and_the_manifest() {
        if let Some(dir) = child_dir() {
            let manifest = dir.join("MANIFEST").display().to_string();
            let error = rlocation("_main/pkg/absent.txt").expect_err("absent");
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
            assert!(
                error.to_string().contains("_main/pkg/absent.txt"),
                "{error}"
            );
            assert!(error.to_string().contains(&manifest), "{error}");
            let stale = rlocation("_main/pkg/stale.txt").expect_err("stale target");
            assert!(stale.to_string().contains("_main/pkg/stale.txt"), "{stale}");
            let text = panic_text("pkg/absent.txt");
            assert!(text.contains("runfile pkg/absent.txt"), "{text}");
            assert!(text.contains("_main/pkg/absent.txt"), "{text}");
            assert!(text.contains(&manifest), "{text}");
            return;
        }
        let dir = scratch("dx-testing-missing-");
        let manifest = write(
            dir.path(),
            "MANIFEST",
            &format!(
                "_main/pkg/stale.txt {}\n",
                dir.path().join("gone.txt").display()
            ),
        );
        in_child(
            "tests::a_missing_key_names_the_input_the_key_and_the_manifest",
            dir.path(),
            &[("RUNFILES_MANIFEST_FILE", &manifest)],
        );
    }

    #[test]
    fn a_child_tool_reads_its_own_runfiles_not_the_parent_runfiles() {
        let probe = process_probe();
        let own = dx_path::Resolver::for_binary(&probe)
            .expect("the probe carries its own runfiles")
            .source()
            .to_path_buf();
        let child = run(
            &probe,
            &[
                "--print-env=RUNFILES_MANIFEST_FILE",
                "--print-env=RUNFILES_DIR",
            ],
            &[],
        )
        .expect("probe runs");
        assert!(child.status.success(), "{}", child.combined());
        let own = own.to_string_lossy().into_owned();
        let lines: Vec<&str> = child.stdout.lines().collect();
        if Path::new(&own).is_dir() {
            assert_eq!(lines, ["<unset>", own.as_str()]);
        } else {
            assert_eq!(lines, [own.as_str(), "<unset>"]);
        }
        for name in ["RUNFILES_MANIFEST_FILE", "RUNFILES_DIR", "TEST_SRCDIR"] {
            if let Ok(parent) = std::env::var(name) {
                assert!(
                    !lines.contains(&parent.as_str()),
                    "{name} leaked: {lines:?}"
                );
            }
        }
    }

    #[test]
    fn expect_helpers_report_missing_and_present() {
        let dir = scratch("dx-testing-");
        let path = write(dir.path(), "f.txt", "alpha\nbeta\n");
        assert!(expect_contains(&path, &["alpha", "beta"]).is_ok());
        assert!(expect_contains(&path, &["alpha", "nope"]).is_err());
        assert!(expect_absent(&path, &["forbidden"]).is_ok());
        assert!(expect_absent(&path, &["alpha"]).is_err());
    }

    #[test]
    fn snapshot_pass_and_fail() {
        let dir = scratch("dx-testing-");
        let expected = write(dir.path(), "expected.txt", "same\n");
        let actual = write(dir.path(), "actual.txt", "same\n");
        assert!(snapshot_diff(&expected, &actual, None).is_ok());
        let drifted = write(dir.path(), "drifted.txt", "other\n");
        assert!(snapshot_diff(&expected, &drifted, None).is_err());
    }

    #[test]
    fn assert_valid_json_checks_the_file_parses() {
        let dir = scratch("dx-testing-");
        let expected = write(dir.path(), "e.json", "{\"b\": 1, \"a\": 2}\n");
        assert!(assert_valid_json(&expected).is_ok());
        let invalid = write(dir.path(), "bad.json", "not json\n");
        assert!(assert_valid_json(&invalid).is_err());
    }

    #[test]
    fn scratch_json_and_run_helpers() {
        let first = mkscratch("dx-helper-").expect("scratch");
        let second = mkscratch("dx-helper-").expect("scratch");
        assert_ne!(first, second);
        let path = first.join("doc.json");
        let value: serde_json::Value =
            serde_json::from_str("{\"tools\": [{\"bin_name\": \"dx\"}]}").expect("parse");
        write_json(&path, &value).expect("write");
        let back = read_json(&path).expect("read");
        assert_eq!(back["tools"][0]["bin_name"], "dx");
        assert!(read_json(&first.join("missing.json")).is_err());
        let echo = run(
            &process_probe(),
            &["--stdout-text=hello"],
            &[("DX_HELPER_CHECK", "1")],
        )
        .expect("run");
        assert!(echo.status.success());
        assert!(echo.combined().contains("hello"));
    }

    #[test]
    fn starlark_const_reads_one_quoted_assignment() {
        let text = "exports_files([\"a.yml\"])\n\nVERSION = \"1.26.6\"\n";
        assert_eq!(starlark_const(text, "VERSION", "versions.bzl"), "1.26.6");
        let empty = "BAZELISK_VERSION = \"\"\n";
        assert_eq!(
            starlark_const(empty, "BAZELISK_VERSION", "versions.bzl"),
            ""
        );
    }

    #[test]
    #[should_panic(expected = "versions.bzl has no MISSING")]
    fn starlark_const_names_the_file_it_read() {
        starlark_const("VERSION = \"1\"\n", "MISSING", "versions.bzl");
    }

    #[test]
    fn workflow_paths_sorts_and_dedups_the_list() {
        assert_eq!(
            workflow_paths(
                ".github/workflows/ci.yml\n.github/workflows/bump.yml .github/workflows/ci.yml",
                "DX_WORKFLOWS"
            ),
            [".github/workflows/bump.yml", ".github/workflows/ci.yml"]
        );
    }

    #[test]
    #[should_panic(expected = "DX_WORKFLOWS names no workflow")]
    fn workflow_paths_rejects_an_empty_list() {
        workflow_paths("  ", "DX_WORKFLOWS");
    }

    #[test]
    #[should_panic(expected = "is not a .github/workflows")]
    fn workflow_paths_rejects_a_path_outside_the_workflows_dir() {
        workflow_paths(".github/actionlint.yaml", "DX_WORKFLOWS");
    }

    #[test]
    fn workflow_paths_accepts_yaml() {
        assert_eq!(
            workflow_paths(".github/workflows/x.yaml", "DX_WORKFLOWS"),
            [".github/workflows/x.yaml"]
        );
    }

    #[test]
    fn an_alive_lock_reads_held_until_its_holder_drops() {
        let dir = scratch("dx-testing-alive-lock");
        let lock = dir.path().join("probe.alive");
        let holder = std::fs::File::create(&lock).expect("lock file");
        holder.try_lock().expect("lock taken");
        assert!(alive_lock_held(&lock));
        drop(holder);
        assert_alive_lock_released(&lock);
        assert!(!alive_lock_held(&lock));
    }
}
