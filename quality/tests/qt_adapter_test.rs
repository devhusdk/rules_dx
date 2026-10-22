use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;
use quality_adapter::commands::{qmlformat_check, qmllint_check, Invocation};
use quality_adapter::parsers::{parse_qmlformat, parse_qmllint};

fn path(variable: &str) -> PathBuf {
    let rel = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
    dx_testing::resolve_runfiles(&rel)
}

fn run(invocation: &Invocation) -> dx_testing::Run {
    let argv: Vec<&str> = invocation
        .argv
        .iter()
        .map(|arg| arg.to_str().expect("utf-8 argument"))
        .collect();
    dx_testing::run(Path::new(argv[0]), &argv[1..], &[]).expect("run tool")
}

#[test]
fn qmllint_output_parses_through_the_adapter() {
    let sample = path("DX_QMLLINT_FIXTURE");
    let name = sample.to_str().expect("utf-8 path");
    let out = run(&qmllint_check(&path("DX_QMLLINT"), &[sample.as_path()]));
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    let report: Value = dx_testing::serde_json::from_str(&out.stdout).expect("JSON report");
    let pinned = dx_testing::read_json(&path("DX_QMLLINT_FIXTURE_DIRTY")).expect("fixture");
    assert_eq!(report["revision"], pinned["revision"]);
    assert_eq!(
        report["files"][0]["warnings"],
        pinned["files"][0]["warnings"]
    );
    let findings =
        parse_qmllint(out.stdout.as_bytes(), out.status.code(), &[name]).expect("parsed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, name);
    let finding = &findings[0].finding;
    assert_eq!(finding.rule_id, "unqualified");
    assert_eq!(finding.message, "Unqualified access");
    assert_eq!((finding.start.line, finding.start.column), (4, 28));
}

#[test]
fn qmlformat_output_parses_through_the_adapter() {
    let sample = path("DX_QMLFORMAT_FIXTURE");
    let name = sample.to_str().expect("utf-8 path");
    let input = std::fs::read(&sample).expect("sample");
    let out = run(&qmlformat_check(&path("DX_QMLFORMAT"), &[sample.as_path()]));
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let findings =
        parse_qmlformat(out.stdout.as_bytes(), out.status.code(), &input, name).expect("parsed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, name);
    let dir = dx_testing::mkscratch("qt-adapter").expect("scratch directory");
    let formatted = dir.join("Formatted.qml");
    std::fs::write(&formatted, &out.stdout).expect("write formatted source");
    let again = run(&qmlformat_check(
        &path("DX_QMLFORMAT"),
        &[formatted.as_path()],
    ));
    let clean = parse_qmlformat(
        again.stdout.as_bytes(),
        again.status.code(),
        out.stdout.as_bytes(),
        name,
    )
    .expect("parsed");
    assert!(clean.is_empty());
}
