use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;
use quality_adapter::commands::{qmlformat_check, qmlformat_fix, qmllint_check, Invocation};
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

const CLEAN: &str = "import QtQuick\n\nItem {\n    property string greet: \"hello\"\n}\n";
const DIRTY: &str = "import QtQuick\nItem {   property string greet:\"hello\"\n}\n";
const MALFORMED: &str = "import QtQuick\nItem { property string greet: \n";

fn scratch_file(name: &str, content: &str) -> PathBuf {
    let dir = dx_testing::mkscratch("qt-adapter").expect("scratch directory");
    let file = dir.join(name);
    std::fs::write(&file, content).expect("write source");
    file
}

fn findings(file: &Path) -> Result<usize, quality_adapter::parsers::ParseError> {
    let input = std::fs::read(file).expect("source");
    let out = run(&qmlformat_check(&path("DX_QMLFORMAT"), &[file]));
    parse_qmlformat(
        out.stdout.as_bytes(),
        out.status.code(),
        &input,
        file.to_str().expect("utf-8 path"),
    )
    .map(|found| found.len())
}

#[test]
fn qmlformat_check_leaves_the_source_untouched() {
    let file = scratch_file("Dirty.qml", DIRTY);
    assert_eq!(findings(&file).expect("parsed"), 1);
    assert_eq!(std::fs::read_to_string(&file).expect("source"), DIRTY);
}

#[test]
fn qmlformat_fix_then_check_round_trips_to_clean() {
    let file = scratch_file("RoundTrip.qml", DIRTY);
    let fixed = run(&qmlformat_fix(&path("DX_QMLFORMAT"), &[file.as_path()]));
    assert_eq!(fixed.status.code(), Some(0), "{}", fixed.combined());
    assert_eq!(std::fs::read_to_string(&file).expect("source"), CLEAN);
    assert_eq!(findings(&file).expect("parsed"), 0);
}

#[test]
fn qmlformat_clean_source_has_no_findings() {
    let file = scratch_file("Clean.qml", CLEAN);
    assert_eq!(findings(&file).expect("parsed"), 0);
}

#[test]
fn qmlformat_malformed_source_is_an_error_not_clean() {
    let file = scratch_file("Malformed.qml", MALFORMED);
    let out = run(&qmlformat_check(&path("DX_QMLFORMAT"), &[file.as_path()]));
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.stdout.is_empty(), "{}", out.stdout);
    assert!(findings(&file).is_err());
    assert_eq!(std::fs::read_to_string(&file).expect("source"), MALFORMED);
}

#[test]
fn qmlformat_ini_next_to_the_source_changes_the_formatting() {
    let file = scratch_file("Configured.qml", CLEAN);
    std::fs::write(
        file.with_file_name(".qmlformat.ini"),
        "[General]\nIndentWidth=2\n",
    )
    .expect("write config");
    assert_eq!(findings(&file).expect("parsed"), 1);
    let out = run(&qmlformat_check(&path("DX_QMLFORMAT"), &[file.as_path()]));
    assert!(
        out.stdout.contains("\n  property string greet"),
        "{}",
        out.stdout
    );
}
