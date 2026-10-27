use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Mutex, PoisonError};

/// Serializes copying the harness and spawning it across this test binary's threads.
static SPAWN_LOCK: Mutex<()> = Mutex::new(());

fn harness() -> PathBuf {
    let rel = std::env::var("STARLARK_HARNESS_BIN")
        .unwrap_or_else(|_| panic!("STARLARK_HARNESS_BIN must name the staged harness"));
    dx_testing::resolve_runfiles(&rel)
}

fn case_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!(
        "starlark-harness-exit-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .unwrap_or_else(|err| panic!("cannot clear {}: {err}", dir.display()));
    }
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|err| panic!("cannot create {}: {err}", dir.display()));
    dir
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("cannot create {}: {err}", parent.display()));
    }
    std::fs::write(path, bytes)
        .unwrap_or_else(|err| panic!("cannot write {}: {err}", path.display()));
}

/// Copies the harness to `exe` and spawns `cmd` as that copy.
///
/// `copy` keeps `exe` open for write until it returns. A fork from another test thread in
/// that window inherits the write fd, and exec of `exe` then fails with ETXTBSY. Holding
/// one lock across the copy and the spawn keeps every fork in this binary out of that window.
fn spawn_copy(exe: &Path, cmd: &mut Command) -> Child {
    let _guard = SPAWN_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    std::fs::copy(harness(), exe)
        .unwrap_or_else(|err| panic!("cannot copy the harness to {}: {err}", exe.display()));
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("cannot run {}: {err}", exe.display()))
}

fn run_case(name: &str, expected: &str, actual: &str) -> Output {
    let dir = case_dir(name);
    let exe = dir.join(format!("probe_test{}", std::env::consts::EXE_SUFFIX));
    let manifest = serde_json::json!({
        "version": 2,
        "name": "//pkg:probe",
        "checks": [
            {"kind": "equal", "name": name, "expected": expected, "actual": actual},
        ],
        "files": [],
        "observations": [],
    });
    write(
        &dir.join("probe_test.runfiles/_main/pkg/case.starlark_manifest.json"),
        manifest.to_string().as_bytes(),
    );
    let mut cmd = Command::new(&exe);
    cmd.env(
        "STARLARK_HARNESS_MANIFEST",
        "_main/pkg/case.starlark_manifest.json",
    )
    .current_dir(&dir);
    let child = spawn_copy(&exe, &mut cmd);
    child
        .wait_with_output()
        .unwrap_or_else(|err| panic!("cannot wait for {}: {err}", exe.display()))
}

#[test]
fn a_failing_check_exits_one_and_prints_the_fail_line() {
    let output = run_case("off", "3", "2");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("FAIL: off"), "{stdout}");
    assert!(
        stdout.contains("starlark_test: 0 passed, 1 failed"),
        "{stdout}"
    );
}

#[test]
fn passing_checks_exit_zero() {
    let output = run_case("sum", "3", "3");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("PASS: sum"), "{stdout}");
    assert!(
        stdout.contains("starlark_test: 1 passed, 0 failed"),
        "{stdout}"
    );
}

#[test]
fn a_missing_manifest_env_exits_one() {
    let dir = case_dir("noenv");
    let exe = dir.join(format!("probe_test{}", std::env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(&exe);
    cmd.env_remove("STARLARK_HARNESS_MANIFEST")
        .current_dir(&dir);
    let child = spawn_copy(&exe, &mut cmd);
    let output = child
        .wait_with_output()
        .unwrap_or_else(|err| panic!("cannot wait for {}: {err}", exe.display()));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty());
    assert!(
        stderr.starts_with("starlark_harness: missing environment"),
        "{stderr}"
    );
}
