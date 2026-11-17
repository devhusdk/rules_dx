use std::path::{Path, PathBuf};

const UPSTREAM: &str = "_main/pkg/upstream";

fn absolute(var: &str) -> PathBuf {
    std::path::absolute(dx_testing::env_file(var)).unwrap_or_else(|error| panic!("{var}: {error}"))
}

fn runfiles_bash() -> PathBuf {
    let found = dx_testing::rlocation("bazel_tools/tools/bash/runfiles/runfiles.bash")
        .expect("runfiles.bash is in the runfiles");
    std::path::absolute(&found).expect("runfiles.bash resolves")
}

struct Launch {
    success: bool,
    stdout: String,
    stderr: String,
}

fn launch(name: &str, key: &str, coverage: Option<&Path>) -> Launch {
    let dir = dx_testing::scratch_dir(name);
    let upstream = std::env::current_exe().expect("the test binary path");
    let manifest = dir.join("MANIFEST");
    dx_testing::write_text(
        &manifest,
        &format!(
            "bazel_tools/tools/bash/runfiles/runfiles.bash {}\n_main/tools/coverage/lib/runfile.sh {}\n{UPSTREAM} {}\n",
            runfiles_bash().display(),
            absolute("DX_RUNFILE_LIB").display(),
            upstream.display(),
        ),
    );
    let mut command = std::process::Command::new(absolute("DX_KCOV_LAUNCHER"));
    command
        .args(["--list"])
        .env("RUNFILES_MANIFEST_FILE", &manifest)
        .env("DX_KCOV_UPSTREAM", key)
        .env_remove("RUNFILES_DIR")
        .env_remove("TEST_SRCDIR")
        .env_remove("RUNFILES_MANIFEST_ONLY")
        .env_remove("COVERAGE_DIR");
    if let Some(coverage) = coverage {
        command.env("COVERAGE_DIR", coverage);
    }
    let output = command.output().expect("kcov_launcher runs");
    Launch {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn ran_upstream(launch: &Launch) -> bool {
    launch.stdout.contains("runfile_reads_the_manifest_entry")
}

#[test]
fn runfile_reads_the_manifest_entry() {
    let launch = launch("runfile_reads_the_manifest_entry", UPSTREAM, None);
    assert!(
        launch.success && ran_upstream(&launch),
        "the upstream did not run through the manifest entry:\n{}{}",
        launch.stdout,
        launch.stderr
    );
}

#[test]
fn runfile_does_not_prefix_match() {
    let launch = launch(
        "runfile_does_not_prefix_match",
        &format!("{UPSTREAM}.bak"),
        None,
    );
    assert!(
        !launch.success && !ran_upstream(&launch),
        "prefix match ran the upstream:\n{}",
        launch.stdout
    );
}

#[test]
fn runfile_returns_nothing_for_a_missing_entry() {
    let launch = launch(
        "runfile_returns_nothing_for_a_missing_entry",
        "_main/pkg/missing",
        None,
    );
    assert!(
        !launch.success && !ran_upstream(&launch),
        "missing entry ran the upstream:\n{}",
        launch.stdout
    );
}

#[test]
fn passthrough_returns_under_coverage() {
    let coverage = dx_testing::scratch_dir("passthrough_returns_under_coverage").join("cov");
    let launch = launch(
        "passthrough_returns_under_coverage_run",
        UPSTREAM,
        Some(&coverage),
    );
    assert!(
        !ran_upstream(&launch),
        "passthrough ran with COVERAGE_DIR set:\n{}",
        launch.stdout
    );
}
