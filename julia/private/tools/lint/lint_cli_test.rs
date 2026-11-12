use dx_testing::{env_file, resolve_runfiles};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(dir: &Path, fixture: &str) -> (Output, PathBuf) {
    let source = resolve_runfiles(&format!(
        "_main/julia/private/tools/lint/testdata/{fixture}"
    ));
    let file = dir.join(fixture);
    std::fs::copy(&source, &file).unwrap_or_else(|error| panic!("copy: {error}"));
    let report = dir.join("report.json");
    let bin = env_file("JULIA_LINT");
    let out = Command::new(bin)
        .arg("--scratch")
        .arg(dir.join("work"))
        .arg("--report")
        .arg(&report)
        .arg(&file)
        .env("TMPDIR", dir)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    (out, report)
}

fn report_of(fixture: &str) -> String {
    let scratch = dx_test_scratch::scratch("julia-lint");
    let (out, report) = run(scratch.path(), fixture);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(report).unwrap()
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
