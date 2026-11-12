use dx_testing::{env_file, resolve_runfiles};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(dir: &Path, fixture: &str) -> (Output, PathBuf) {
    let source = resolve_runfiles(&format!("_main/julia/private/tools/jet/testdata/{fixture}"));
    std::fs::copy(&source, dir.join(fixture)).unwrap_or_else(|error| panic!("copy: {error}"));
    let config = dir.join("config.toml");
    std::fs::write(&config, format!("includes = []\nroots = [\"{fixture}\"]\n"))
        .unwrap_or_else(|error| panic!("config: {error}"));
    let report = dir.join("report.json");
    let bin = env_file("JULIA_JET");
    let out = Command::new(bin)
        .arg("--config")
        .arg(&config)
        .arg("--scratch")
        .arg(dir)
        .arg("--report")
        .arg(&report)
        .env("TMPDIR", dir)
        .output()
        .unwrap_or_else(|error| panic!("spawn: {error}"));
    (out, report)
}

#[test]
fn clean_file_gives_an_empty_report() {
    let scratch = dx_test_scratch::scratch("julia-jet");
    let (out, report) = run(scratch.path(), "clean.jl");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(std::fs::read_to_string(report).unwrap(), "");
}

#[test]
fn undefined_global_is_reported_at_its_line() {
    let scratch = dx_test_scratch::scratch("julia-jet");
    let (out, report) = run(scratch.path(), "buggy.jl");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = std::fs::read_to_string(report).unwrap();
    assert!(body.contains("\"v\":1"), "{body}");
    assert!(body.contains("\"kind\":\"UndefVarError\""), "{body}");
    assert!(body.contains("buggy.jl\",\"line\":3"), "{body}");
}

#[test]
fn missing_arguments_are_a_tool_failure() {
    let bin = env_file("JULIA_JET");
    let out = Command::new(bin).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
