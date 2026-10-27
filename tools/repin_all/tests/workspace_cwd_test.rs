//! Runs repin-all against a fake bazel and checks where it runs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repin_all() -> PathBuf {
    let rel = std::env::var("DX_REPIN_ALL").expect("DX_REPIN_ALL must be set");
    dx_testing::resolve_runfiles(&rel)
}

#[cfg(unix)]
fn fake_bazel(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("bazel");
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s\\n' \"$(pwd)\" \"$*\" > \"$FAKE_BAZEL_LOG\"\nexit 7\n",
    )
    .expect("fake bazel");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[cfg(windows)]
fn fake_bazel(dir: &Path) -> PathBuf {
    let path = dir.join("bazel.cmd");
    std::fs::write(
        &path,
        "@echo off\r\n> \"%FAKE_BAZEL_LOG%\" echo %CD%\r\n>> \"%FAKE_BAZEL_LOG%\" echo %*\r\nexit /b 7\r\n",
    )
    .expect("fake bazel");
    path
}

fn run_wrapper(scratch: &Path, workspace: Option<&Path>, log: &Path) -> Output {
    let elsewhere = scratch.join("runfiles");
    std::fs::create_dir_all(&elsewhere).expect("runfiles dir");
    let mut command = Command::new(repin_all());
    command
        .arg("--extra")
        .current_dir(&elsewhere)
        .env("DX_BAZEL_BIN", fake_bazel(scratch))
        .env("FAKE_BAZEL_LOG", log)
        .env_remove("BUILD_WORKSPACE_DIRECTORY");
    if let Some(workspace) = workspace {
        command.env("BUILD_WORKSPACE_DIRECTORY", workspace);
    }
    command.output().expect("repin_all runs")
}

#[test]
fn nested_bazel_runs_in_the_build_workspace_directory() {
    let scratch_tmp = dx_test_scratch::scratch("repin-cwd");
    let scratch = scratch_tmp.path().to_path_buf();
    let workspace = scratch.join("checkout");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let log = scratch.join("bazel.log");
    let output = run_wrapper(&scratch, Some(&workspace), &log);
    assert_eq!(output.status.code(), Some(7), "exit code passes through");
    let recorded = std::fs::read_to_string(&log).expect("fake bazel ran");
    let mut lines = recorded.lines();
    let cwd = PathBuf::from(lines.next().expect("cwd line"));
    assert_eq!(
        cwd.canonicalize().expect("cwd"),
        workspace.canonicalize().expect("workspace")
    );
    assert_eq!(
        lines.next().map(str::trim_end),
        Some("--nohome_rc --nosystem_rc run //cli/cli:dx -- update --extra"),
        "forwards to dx update"
    );
}

#[test]
fn unset_build_workspace_directory_fails_without_running_bazel() {
    let scratch_tmp = dx_test_scratch::scratch("repin-unset");
    let scratch = scratch_tmp.path().to_path_buf();
    let log = scratch.join("bazel.log");
    let output = run_wrapper(&scratch, None, &log);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BUILD_WORKSPACE_DIRECTORY is unset"),
        "stderr: {stderr}"
    );
    assert!(!log.exists(), "fake bazel must not run");
}
