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

/// Runs one tool, pointing it at the runfiles that sit beside it.
///
/// A tool reads its own runfiles from `argv[0]` and the environment it inherits, and a test
/// hands its child the test's own tree. A child staged somewhere else then looks for its
/// runfiles where they were never built. The manifest beside the child describes it exactly,
/// so name that and take the inherited tree away.
fn command_for(bin: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(bin);
    if let Some(manifest) = dx_path::manifest_beside(bin) {
        command.env("RUNFILES_MANIFEST_FILE", manifest);
        command.env_remove("RUNFILES_DIR");
    }
    command
}

pub fn run(bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::io::Result<Run> {
    let output = command_for(bin)
        .args(args)
        .envs(envs.iter().copied())
        .output()?;
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

fn candidates(rel: &Path) -> Vec<PathBuf> {
    let mut out = vec![rel.to_path_buf()];
    if rel.extension().is_none() {
        out.extend(
            dx_path::host::EXECUTABLE_SUFFIXES
                .iter()
                .copied()
                .chain(suffixes().iter().copied())
                .map(|suffix| {
                    let mut name = rel.as_os_str().to_os_string();
                    name.push(suffix);
                    PathBuf::from(name)
                }),
        );
    }
    out
}

fn first_existing(base: &Path) -> Option<PathBuf> {
    candidates(base).into_iter().find(|path| path.exists())
}

fn manifest_lookup(root: &Path, workspace: &str, rel: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(root.join("MANIFEST")).ok()?;
    for candidate in candidates(rel) {
        let prefix = format!("{workspace}/{} ", dx_path::posix(&candidate));
        if let Some(entry) = text.lines().find(|line| line.starts_with(&prefix)) {
            let resolved = Path::new(entry[prefix.len()..].trim());
            if resolved.exists() {
                return Some(resolved.to_path_buf());
            }
        }
    }
    None
}

pub fn resolve_runfiles(rel: &str) -> PathBuf {
    let direct = PathBuf::from(rel);
    if direct.is_absolute() {
        return direct;
    }
    let rel = Path::new(rel);
    let workspace = std::env::var("TEST_WORKSPACE").unwrap_or_else(|_| "_main".to_owned());
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = first_existing(&cwd.join(rel)) {
            return found;
        }
    }
    for key in ["TEST_SRCDIR", "RUNFILES_DIR"] {
        let Ok(root) = std::env::var(key) else {
            continue;
        };
        let root = PathBuf::from(root);
        for prefix in [workspace.as_str(), "rules_dx", "_main"] {
            if let Some(found) = first_existing(&root.join(prefix).join(rel)) {
                return found;
            }
        }
        for prefix in [workspace.as_str(), "rules_dx", "_main"] {
            if let Some(found) = manifest_lookup(&root, prefix, rel) {
                return found;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        return cwd.join(rel);
    }
    direct
}

/// Names the Bazel-built process probe from the runfiles of the calling test.
pub fn process_probe() -> PathBuf {
    let rel = std::env::var("DX_PROCESS_PROBE")
        .unwrap_or_else(|_| panic!("DX_PROCESS_PROBE must name the process probe"));
    resolve_runfiles(&rel)
}

pub fn runfiles_root() -> PathBuf {
    let root =
        std::env::var("TEST_SRCDIR").unwrap_or_else(|_| panic!("TEST_SRCDIR is set under Bazel"));
    let workspace = std::env::var("TEST_WORKSPACE")
        .unwrap_or_else(|_| panic!("TEST_WORKSPACE is set under Bazel"));
    Path::new(&root).join(workspace)
}

pub fn read_runfiles(rel: &str) -> String {
    let path = runfiles_root().join(rel);
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

pub fn workflow_exports(text: &str, file: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut in_exports = false;
    for line in text.lines() {
        if line.starts_with("exports_files(") {
            in_exports = true;
            continue;
        }
        if !in_exports {
            continue;
        }
        if !line.starts_with(' ') {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('"') {
            continue;
        }
        let name = trimmed
            .trim_start_matches('"')
            .split('"')
            .next()
            .unwrap_or_default();
        if name.ends_with(".yml") {
            names.push(name.to_owned());
        }
    }
    assert!(!names.is_empty(), "{file} exports no workflow");
    names
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
            let mut hint = format!("staged refreshed golden at {}", staged.display());
            if let Some(rel) = workspace_rel {
                hint.push_str(&format!("; copy it to {rel} in the checkout, then re-run"));
            }
            return Ok(());
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
    fn runfiles_resolution_prefers_an_existing_file() {
        let dir = scratch("dx-testing-runfiles-");
        let direct = write(dir.path(), "pkg/tool", "#!/bin/sh\n");
        assert_eq!(resolve_runfiles(&direct.to_string_lossy()), direct);
    }

    #[test]
    fn runfiles_resolution_reads_the_manifest() {
        if let Some(raw) = std::env::var_os("DX_TESTING_MANIFEST_CHILD") {
            let raw = raw.to_string_lossy().into_owned();
            let (mode, base) = raw.split_once(':').expect("mode:path");
            let base = PathBuf::from(base);
            if mode == "panic" {
                unsafe {
                    std::env::set_var("TEST_SRCDIR", &base);
                }
                panic!("intentional child failure");
            }
            let real = write(&base, "real/tool", "#!/bin/sh\n");
            write(
                &base,
                "MANIFEST",
                &format!("_main/pkg/tool {}\n", real.display()),
            );
            unsafe {
                std::env::set_var("TEST_SRCDIR", &base);
            }
            let resolved = resolve_runfiles("pkg/tool");
            unsafe {
                std::env::remove_var("TEST_SRCDIR");
            }
            assert_eq!(resolved, real);
            std::fs::write(base.join("ran"), b"ok").expect("sentinel");
            return;
        }
        let dir = scratch("dx-testing-manifest-");
        let before = std::env::var_os("TEST_SRCDIR");
        let spawn = |mode: &str| {
            std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args(["tests::runfiles_resolution_reads_the_manifest", "--exact"])
                .env(
                    "DX_TESTING_MANIFEST_CHILD",
                    format!("{mode}:{}", dir.path().display()),
                )
                .output()
                .expect("spawn manifest child")
        };
        let poisoned = spawn("panic");
        assert!(
            !poisoned.status.success(),
            "panicking child must fail: {}",
            String::from_utf8_lossy(&poisoned.stdout)
        );
        assert_eq!(
            std::env::var_os("TEST_SRCDIR"),
            before,
            "child env mutation dies with the child"
        );
        let ok = spawn("run");
        assert!(
            ok.status.success(),
            "child failed: {}{}",
            String::from_utf8_lossy(&ok.stdout),
            String::from_utf8_lossy(&ok.stderr)
        );
        assert!(
            dir.path().join("ran").exists(),
            "child ran the assertions"
        );
        assert_eq!(
            std::env::var_os("TEST_SRCDIR"),
            before,
            "child env mutation dies with the child"
        );
        dir.close().expect("cleanup");
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
    fn workflow_exports_lists_only_the_exported_workflows() {
        let build = "exports_files([\n    \".bazelignore\",\n    \"workflows/bump.yml\",\n    \"workflows/ci.yml\",\n])\n\nfilegroup(\n    name = \"later\",\n    srcs = [\"workflows/ghcr.yml\"],\n)\n";
        assert_eq!(
            workflow_exports(build, ".github/BUILD.bazel"),
            ["workflows/bump.yml", "workflows/ci.yml"]
        );
    }

    #[test]
    #[should_panic(expected = "exports no workflow")]
    fn workflow_exports_rejects_a_build_that_exports_none() {
        workflow_exports(
            "exports_files([\n    \".bazelignore\",\n])\n",
            ".github/BUILD.bazel",
        );
    }
}
