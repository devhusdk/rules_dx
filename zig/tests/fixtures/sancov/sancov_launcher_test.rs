//! Runs the dx-runtime sancov helper over a -ffuzz Zig test and checks the LCOV it writes.
use std::path::Path;

fn launch(coverage: Option<&Path>) -> dx_testing::Run {
    let launcher = std::path::absolute(dx_testing::env_file("DX_RUNTIME"))
        .expect("the launcher path resolves");
    let mut envs = vec![
        (
            "DX_LLVM_SYMBOLIZER",
            dx_testing::env_text("DX_LLVM_SYMBOLIZER"),
        ),
        ("DX_RUNTIME_TOOL", "sancov".to_owned()),
        (
            "DX_SANCOV_SOURCES",
            dx_testing::env_text("DX_SANCOV_SOURCES"),
        ),
        (
            "DX_SANCOV_UPSTREAM",
            dx_testing::env_text("DX_SANCOV_UPSTREAM"),
        ),
    ];
    if let Some(dir) = coverage {
        envs.push(("COVERAGE_DIR", dx_testing::path_text(dir)));
    }
    // Runs with the test's runfiles, as Bazel runs the launcher as the test executable.
    let mut command = std::process::Command::new(&launcher);
    command.envs(envs);
    dx_testing::run_command(&mut command, std::time::Duration::from_secs(120))
        .expect("the launcher runs")
}

#[test]
fn writes_lcov_with_every_greet_line_run() {
    let dir = dx_testing::scratch_dir("sancov_coverage");
    let run = launch(Some(&dir));
    assert!(run.status.success(), "launcher failed:\n{}", run.combined());
    let report = dx_testing::read_text(&dir.join("sancov.dat"));
    assert!(
        report.starts_with("SF:zig/tests/conformance/greet.zig\n"),
        "{report}"
    );
    let lines: Vec<&str> = report.lines().filter(|l| l.starts_with("DA:")).collect();
    assert!(!lines.is_empty(), "{report}");
    assert!(lines.iter().all(|l| !l.ends_with(",0")), "{report}");
    assert_eq!(report.matches("SF:").count(), 1, "{report}");
}

#[test]
fn only_runs_the_test_without_coverage_dir() {
    let run = launch(None);
    assert!(run.status.success(), "launcher failed:\n{}", run.combined());
    assert!(run.stderr.contains("greets...OK"), "{}", run.combined());
}
