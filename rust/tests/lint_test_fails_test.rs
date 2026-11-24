#[test]
fn lint_check_fails_on_the_dirty_fixture() {
    let key = std::env::var("DX_LINT_BINARY").expect("DX_LINT_BINARY must be set");
    let binary = dx_testing::rlocation(&key).expect("the lint binary must resolve from runfiles");
    let run = dx_testing::run(&binary, &[], &[]).expect("the lint binary must start");
    let stderr = &run.stderr;
    assert_eq!(run.status.code(), Some(1), "stderr:\n{stderr}");
    assert!(
        stderr.contains("rust/tests/conformance/lint_dirty.rs:5:5: clippy::len_zero: "),
        "stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("clippy_findings: 1 finding(s)"),
        "stderr:\n{stderr}"
    );
}
