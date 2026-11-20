#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
use dx_testing::{env_file, resolve_runfiles};
use std::process::Command;
use std::sync::OnceLock;

const FIXTURES: [&str; 3] = ["buggy.jl", "clean.jl", "unicode.jl"];

fn lint_all() -> String {
    let scratch = dx_test_scratch::scratch("julia-lint").unwrap();
    let dir = scratch.path();
    let files: Vec<_> = FIXTURES
        .iter()
        .map(|fixture| {
            let source = resolve_runfiles(&format!(
                "_main/julia/private/tools/lint/testdata/{fixture}"
            ));
            let file = dir.join(fixture);
            std::fs::copy(&source, &file).unwrap_or_else(|error| panic!("copy: {error}"));
            file
        })
        .collect();
    let report = dir.join("report.json");
    let out = Command::new(env_file("JULIA_LINT"))
        .arg("--scratch")
        .arg(dir.join("work"))
        .arg("--report")
        .arg(&report)
        .args(&files)
        .env("TMPDIR", dir)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(report).unwrap()
}

fn report_of(fixture: &str) -> String {
    static REPORT: OnceLock<String> = OnceLock::new();
    let file = format!("{fixture}\",");
    REPORT
        .get_or_init(lint_all)
        .lines()
        .filter(|line| line.contains(&file))
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn clean_file_gives_an_empty_report() {
    assert_eq!(report_of("clean.jl"), "");
}

#[test]
fn findings_carry_rule_and_position() {
    let body = report_of("buggy.jl");
    assert!(body.contains("\"v\":1"), "{body}");
    assert!(body.contains("\"rule\":\"index_from_length\""), "{body}");
    assert!(body.contains("\"line\":3,\"col\":9"), "{body}");
    assert!(body.contains("\"rule\":\"const_if_condition\""), "{body}");
}

#[test]
fn columns_count_characters_not_bytes() {
    let body = report_of("unicode.jl");
    assert!(body.contains("\"line\":3,\"col\":17"), "{body}");
}

#[test]
fn missing_arguments_are_a_tool_failure() {
    let bin = env_file("JULIA_LINT");
    let out = Command::new(bin).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
