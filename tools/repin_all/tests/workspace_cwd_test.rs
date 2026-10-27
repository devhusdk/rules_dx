//! Runs repin-all against a fake dx in a scratch runfiles tree and checks how it launches dx.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repin_all() -> PathBuf {
    let rel = std::env::var("DX_REPIN_ALL").expect("DX_REPIN_ALL must be set");
    dx_testing::resolve_runfiles(&rel)
}

#[cfg(unix)]
const DX_KEY: &str = "_main/cli/cli/dx";
#[cfg(windows)]
const DX_KEY: &str = "_main/cli/cli/dx.cmd";

#[cfg(unix)]
fn install_fake_dx(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(
        path,
        "#!/bin/sh\n{ pwd; echo \"$*\"; echo \"${RUNFILES_DIR:-unset}|${RUNFILES_MANIFEST_FILE:-unset}|${BUILD_WORKING_DIRECTORY:-unset}\"; } > \"$FAKE_DX_LOG\"\nexit 7\n",
    )
    .expect("fake dx");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(windows)]
fn install_fake_dx(path: &Path) {
    std::fs::write(
        path,
        "@echo off\r\n> \"%FAKE_DX_LOG%\" echo %CD%\r\n>> \"%FAKE_DX_LOG%\" echo %*\r\n>> \"%FAKE_DX_LOG%\" echo %RUNFILES_DIR%^|%RUNFILES_MANIFEST_FILE%^|%BUILD_WORKING_DIRECTORY%\r\nexit /b 7\r\n",
    )
    .expect("fake dx");
}

struct Setup {
    runfiles: PathBuf,
    workspace: PathBuf,
    log: PathBuf,
}

fn setup(scratch: &Path) -> Setup {
    let runfiles = scratch.join("runfiles");
    let dx = runfiles.join(DX_KEY);
    std::fs::create_dir_all(dx.parent().expect("parent")).expect("runfiles tree");
    install_fake_dx(&dx);
    let workspace = scratch.join("checkout");
    std::fs::create_dir_all(&workspace).expect("workspace");
    Setup {
        runfiles,
        workspace,
        log: scratch.join("dx.log"),
    }
}

fn wrapper(setup: &Setup, args: &[&str]) -> Command {
    let mut command = Command::new(repin_all());
    command
        .args(args)
        .current_dir(&setup.runfiles)
        .env("RUNFILES_DIR", &setup.runfiles)
        .env("JAVA_RUNFILES", &setup.runfiles)
        .env_remove("RUNFILES_MANIFEST_FILE")
        .env("DX_REPIN_DX", DX_KEY)
        .env("FAKE_DX_LOG", &setup.log)
        .env("BUILD_WORKSPACE_DIRECTORY", &setup.workspace)
        .env("BUILD_WORKING_DIRECTORY", &setup.workspace)
        .env_remove("BUILD_EXECROOT");
    command
}

fn recorded(setup: &Setup) -> Vec<String> {
    std::fs::read_to_string(&setup.log)
        .expect("fake dx ran")
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

fn assert_code(output: &Output, code: i32) {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
}

#[test]
fn dx_runs_in_the_workspace_with_the_outer_output_base() {
    let scratch_tmp = dx_test_scratch::scratch("repin-cwd");
    let scratch = scratch_tmp.path().to_path_buf();
    let setup = setup(&scratch);
    let output_base = scratch.join("ob");
    let execroot = output_base.join("execroot").join("_main");
    let output = wrapper(&setup, &["--extra"])
        .env("BUILD_EXECROOT", &execroot)
        .output()
        .expect("repin_all runs");
    assert_code(&output, 7);
    let lines = recorded(&setup);
    assert_eq!(
        PathBuf::from(&lines[0]).canonicalize().expect("cwd"),
        setup.workspace.canonicalize().expect("workspace")
    );
    assert_eq!(
        lines[1],
        format!(
            "update --bazel-startup-option=--output_base={} --extra",
            output_base.display()
        )
    );
    assert!(
        lines[2].starts_with("unset|unset|"),
        "runfiles env must not leak: {}",
        lines[2]
    );
    assert!(
        !lines[2].ends_with("|unset"),
        "BUILD_WORKING_DIRECTORY kept"
    );
}

#[test]
fn no_execroot_adds_no_startup_option() {
    let scratch_tmp = dx_test_scratch::scratch("repin-noexec");
    let setup = setup(scratch_tmp.path());
    let output = wrapper(&setup, &["--extra"]).output().expect("runs");
    assert_code(&output, 7);
    assert_eq!(recorded(&setup)[1], "update --extra");
}

#[test]
fn user_output_base_is_not_overridden() {
    let scratch_tmp = dx_test_scratch::scratch("repin-user-ob");
    let scratch = scratch_tmp.path().to_path_buf();
    let setup = setup(&scratch);
    let execroot = scratch.join("ob").join("execroot").join("_main");
    let output = wrapper(&setup, &["--bazel-startup-option=--output_base=/u"])
        .env("BUILD_EXECROOT", &execroot)
        .output()
        .expect("runs");
    assert_code(&output, 7);
    assert_eq!(
        recorded(&setup)[1],
        "update --bazel-startup-option=--output_base=/u"
    );
}

#[test]
fn unset_build_workspace_directory_fails_without_running_dx() {
    let scratch_tmp = dx_test_scratch::scratch("repin-unset");
    let setup = setup(scratch_tmp.path());
    let output = wrapper(&setup, &[])
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .output()
        .expect("runs");
    assert_code(&output, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BUILD_WORKSPACE_DIRECTORY is unset"),
        "stderr: {stderr}"
    );
    assert!(!setup.log.exists(), "dx must not run");
}

#[test]
fn unresolved_dx_fails_without_running_it() {
    let scratch_tmp = dx_test_scratch::scratch("repin-nodx");
    let setup = setup(scratch_tmp.path());
    for value in [None, Some("_main/missing/dx")] {
        let mut command = wrapper(&setup, &[]);
        match value {
            Some(value) => command.env("DX_REPIN_DX", value),
            None => command.env_remove("DX_REPIN_DX"),
        };
        let output = command.output().expect("runs");
        assert_code(&output, 2);
        assert!(!setup.log.exists(), "dx must not run");
    }
}
