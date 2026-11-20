#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
//! Runs the kcov launcher as a consumer does: rules_dx is `rules_dx+`, kcov is the
//! static binary the release ships and `to_lcov` is the dx-runtime helper.
use std::path::PathBuf;

const REPO: &str = "rules_dx+";
const SOURCE: &str = "tools/coverage/kcov/smoke.c";

fn absolute(var: &str) -> PathBuf {
    std::path::absolute(dx_testing::env_file(var)).unwrap_or_else(|error| panic!("{var}: {error}"))
}

#[test]
fn launcher_writes_lcov_with_the_release_tools() {
    let dir = dx_testing::scratch_dir("release_launcher");
    let runfiles_bash = dx_testing::rlocation("bazel_tools/tools/bash/runfiles/runfiles.bash")
        .expect("runfiles.bash is in the runfiles");
    let mapping = dir.join("_repo_mapping");
    dx_testing::write_text(
        &mapping,
        &format!(",rules_dx,{REPO}\n{REPO},rules_dx,{REPO}\n"),
    );
    let sources = dir.join("sources");
    dx_testing::write_text(&sources, &format!("{SOURCE}\n"));
    let entries = [
        ("_repo_mapping".to_owned(), mapping),
        (
            "bazel_tools/tools/bash/runfiles/runfiles.bash".to_owned(),
            std::path::absolute(&runfiles_bash).expect("runfiles.bash resolves"),
        ),
        (
            format!("{REPO}/tools/coverage/lib/runfile.sh"),
            absolute("DX_RUNFILE_LIB"),
        ),
        (format!("{REPO}/kcov"), absolute("DX_KCOV_STATIC")),
        (format!("{REPO}/smoke"), absolute("DX_KCOV_SMOKE")),
        (format!("{REPO}/smoke.kcov_sources"), sources),
        (format!("{REPO}/dx_runtime"), absolute("DX_RUNTIME")),
    ];
    let manifest = dir.join("MANIFEST");
    dx_testing::write_text(
        &manifest,
        &entries
            .iter()
            .map(|(key, path)| format!("{key} {}\n", path.display()))
            .collect::<String>(),
    );
    let coverage = dir.join("coverage");
    std::fs::create_dir_all(&coverage).expect("coverage dir");
    let tmp = dir.join("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp dir");
    let output = std::process::Command::new(absolute("DX_KCOV_LAUNCHER"))
        .env("RUNFILES_MANIFEST_FILE", &manifest)
        .env("DX_KCOV", format!("{REPO}/kcov"))
        .env("DX_KCOV_UPSTREAM", format!("{REPO}/smoke"))
        .env("DX_KCOV_SOURCES", format!("{REPO}/smoke.kcov_sources"))
        .env("DX_TO_LCOV", format!("{REPO}/dx_runtime"))
        .env("COVERAGE_DIR", &coverage)
        .env("TEST_TMPDIR", &tmp)
        .env_remove("DX_RUNTIME_TOOL")
        .env_remove("RUNFILES_DIR")
        .env_remove("TEST_SRCDIR")
        .env_remove("RUNFILES_MANIFEST_ONLY")
        .output()
        .expect("kcov_launcher runs");
    let lcov = std::fs::read_to_string(coverage.join("kcov.dat")).unwrap_or_default();
    assert!(
        output.status.success(),
        "kcov_launcher failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        lcov.contains(&format!("SF:{SOURCE}\n")),
        "no {SOURCE} in\n{lcov}"
    );
    let hit = lcov
        .lines()
        .filter_map(|line| line.strip_prefix("DA:"))
        .any(|da| da.split(',').nth(1).is_some_and(|n| n != "0"));
    assert!(hit, "no covered line in\n{lcov}");
}
