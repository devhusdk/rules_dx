#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
fn collect(coverage: Option<&std::path::Path>) -> Option<i32> {
    let collect =
        std::path::absolute(dx_testing::env_file("DX_COLLECT")).expect("collect resolves");
    let mut command = std::process::Command::new(&collect);
    command
        .env(
            "DX_BISECT_UPSTREAM",
            dx_testing::env_text("DX_COLLECT_UPSTREAM"),
        )
        .env_remove("COVERAGE_DIR");
    if let Some(coverage) = coverage {
        command.env("COVERAGE_DIR", coverage);
    }
    command
        .status()
        .unwrap_or_else(|error| panic!("{}: {error}", collect.display()))
        .code()
}

#[test]
fn collect_passes_the_upstream_status_without_coverage() {
    assert_eq!(collect(None), Some(3));
}

#[test]
fn collect_passes_the_upstream_status_with_no_data() {
    let coverage = dx_testing::scratch_dir("collect_test").join("cov");
    assert_eq!(collect(Some(&coverage)), Some(3));
}
