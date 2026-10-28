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
    let out = run(&qmllint_check(
        &path("DX_QMLLINT"),
        &[sample.as_path()],
        &[],
        false,
    ));
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
    let out = run(&qmlformat_check(
        &path("DX_QMLFORMAT"),
        &[sample.as_path()],
        None,
    ));
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let findings =
        parse_qmlformat(out.stdout.as_bytes(), out.status.code(), &input, name).expect("parsed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, name);
    let dir_guard = dx_test_scratch::scratch("qt-adapter");
    let dir = dir_guard.path();
    let formatted = dir.join("Formatted.qml");
    std::fs::write(&formatted, &out.stdout).expect("write formatted source");
    let again = run(&qmlformat_check(
        &path("DX_QMLFORMAT"),
        &[formatted.as_path()],
        None,
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

fn scratch_file(name: &str, content: &str) -> (dx_test_scratch::TempDir, PathBuf) {
    let guard = dx_test_scratch::scratch("qt-adapter");
    let file = guard.path().join(name);
    std::fs::write(&file, content).expect("write source");
    (guard, file)
}

fn findings(file: &Path) -> Result<usize, quality_adapter::parsers::ParseError> {
    let input = std::fs::read(file).expect("source");
    let out = run(&qmlformat_check(&path("DX_QMLFORMAT"), &[file], None));
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
    let (_file_dir, file) = scratch_file("Dirty.qml", DIRTY);
    assert_eq!(findings(&file).expect("parsed"), 1);
    assert_eq!(std::fs::read_to_string(&file).expect("source"), DIRTY);
}

#[test]
fn qmlformat_fix_then_check_round_trips_to_clean() {
    let (_file_dir, file) = scratch_file("RoundTrip.qml", DIRTY);
    let fixed = run(&qmlformat_fix(
        &path("DX_QMLFORMAT"),
        &[file.as_path()],
        None,
    ));
    assert_eq!(fixed.status.code(), Some(0), "{}", fixed.combined());
    assert_eq!(std::fs::read_to_string(&file).expect("source"), CLEAN);
    assert_eq!(findings(&file).expect("parsed"), 0);
}

#[test]
fn qmlformat_clean_source_has_no_findings() {
    let (_file_dir, file) = scratch_file("Clean.qml", CLEAN);
    assert_eq!(findings(&file).expect("parsed"), 0);
}

#[test]
fn qmlformat_malformed_source_is_an_error_not_clean() {
    let (_file_dir, file) = scratch_file("Malformed.qml", MALFORMED);
    let out = run(&qmlformat_check(
        &path("DX_QMLFORMAT"),
        &[file.as_path()],
        None,
    ));
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.stdout.is_empty(), "{}", out.stdout);
    assert!(findings(&file).is_err());
    assert_eq!(std::fs::read_to_string(&file).expect("source"), MALFORMED);
}

#[test]
fn qmlformat_ini_next_to_the_source_changes_the_formatting() {
    let (_file_dir, file) = scratch_file("Configured.qml", CLEAN);
    std::fs::write(
        file.with_file_name(".qmlformat.ini"),
        "[General]\nIndentWidth=2\n",
    )
    .expect("write config");
    assert_eq!(findings(&file).expect("parsed"), 1);
    let out = run(&qmlformat_check(
        &path("DX_QMLFORMAT"),
        &[file.as_path()],
        None,
    ));
    assert!(
        out.stdout.contains("\n  property string greet"),
        "{}",
        out.stdout
    );
}

fn lint(
    files: &[&Path],
    import_dirs: &[&Path],
    use_settings: bool,
) -> (
    dx_testing::Run,
    Result<Vec<quality_adapter::parsers::FileFinding>, quality_adapter::parsers::ParseError>,
) {
    let out = run(&qmllint_check(
        &path("DX_QMLLINT"),
        files,
        import_dirs,
        use_settings,
    ));
    let names: Vec<&str> = files
        .iter()
        .map(|file| file.to_str().expect("utf-8 path"))
        .collect();
    let parsed = parse_qmllint(out.stdout.as_bytes(), out.status.code(), &names);
    (out, parsed)
}

#[test]
fn qmllint_clean_source_exits_zero_with_no_findings() {
    let (_file_dir, file) = scratch_file("Clean.qml", CLEAN);
    let (out, parsed) = lint(&[file.as_path()], &[], false);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(parsed.expect("parsed").is_empty());
}

#[test]
fn qmllint_syntax_error_is_a_located_finding() {
    let (_file_dir, file) = scratch_file(
        "Syntax.qml",
        "import QtQuick\nItem {\n    property int x: 1 +\n}\n",
    );
    let (out, parsed) = lint(&[file.as_path()], &[], false);
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    let found = parsed.expect("parsed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].finding.rule_id, "syntax");
    assert_eq!(found[0].finding.start.line, 4);
}

#[test]
fn qmllint_reports_every_file_of_a_batch() {
    let (_clean_dir, clean) = scratch_file("A.qml", CLEAN);
    let (_dirty_dir, dirty) = scratch_file("B.qml", "import QtQuick\nItem { property string s: foo }\n");
    let (out, parsed) = lint(&[clean.as_path(), dirty.as_path()], &[], false);
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    let found = parsed.expect("parsed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file, dirty.to_str().expect("utf-8 path"));
}

#[test]
fn qmllint_unknown_import_is_reported_not_ignored() {
    let (_file_dir, file) = scratch_file("Import.qml", "import Nope.Missing\nItem {}\n");
    let (out, parsed) = lint(&[file.as_path()], &[], false);
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    let found = parsed.expect("parsed");
    assert!(found.iter().all(|item| item.finding.rule_id == "import"));
    assert!(found.iter().any(|item| item
        .finding
        .message
        .contains("Failed to import Nope.Missing")));
}

#[test]
fn qmllint_missing_file_is_an_infrastructure_failure() {
    let dir_guard = dx_test_scratch::scratch("qt-adapter");
    let dir = dir_guard.path();
    let (out, parsed) = lint(&[dir.join("Gone.qml").as_path()], &[], false);
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    let err = parsed.expect_err("no location");
    assert!(err.to_string().contains("Failed to open file"), "{err}");
}

#[test]
fn qmllint_declared_import_dir_resolves_a_module() {
    let dir_guard = dx_test_scratch::scratch("qt-adapter");
    let dir = dir_guard.path();
    let module = dir.join("mods/Foo");
    std::fs::create_dir_all(&module).expect("module dir");
    std::fs::write(module.join("qmldir"), "module Foo\nBar 1.0 Bar.qml\n").expect("qmldir");
    std::fs::write(
        module.join("Bar.qml"),
        "import QtQuick\nItem { property int v: 1 }\n",
    )
    .expect("Bar.qml");
    let user = dir.join("Use.qml");
    std::fs::write(
        &user,
        "import QtQuick\nimport Foo\nBar { property int w: v }\n",
    )
    .expect("Use.qml");
    let roots = dir.join("mods");

    let (out, parsed) = lint(&[user.as_path()], &[], false);
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
    assert!(parsed
        .expect("parsed")
        .iter()
        .any(|item| item.finding.message.contains("Failed to import Foo")));

    let (out, parsed) = lint(&[user.as_path()], &[roots.as_path()], false);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(parsed.expect("parsed").is_empty());
}

#[test]
fn qmllint_ignores_the_host_import_path_variable() {
    let dir_guard = dx_test_scratch::scratch("qt-adapter");
    let dir = dir_guard.path();
    let module = dir.join("mods/Foo");
    std::fs::create_dir_all(&module).expect("module dir");
    std::fs::write(module.join("qmldir"), "module Foo\nBar 1.0 Bar.qml\n").expect("qmldir");
    std::fs::write(module.join("Bar.qml"), "import QtQuick\nItem {}\n").expect("Bar.qml");
    let user = dir.join("Use.qml");
    std::fs::write(&user, "import QtQuick\nimport Foo\nBar {}\n").expect("Use.qml");
    let invocation = qmllint_check(&path("DX_QMLLINT"), &[user.as_path()], &[], false);
    let argv: Vec<&str> = invocation
        .argv
        .iter()
        .map(|arg| arg.to_str().expect("utf-8 argument"))
        .collect();
    let mods = dir.join("mods");
    let out = dx_testing::run(
        Path::new(argv[0]),
        &argv[1..],
        &[
            ("QML_IMPORT_PATH", mods.to_str().expect("utf-8 path")),
            ("QML2_IMPORT_PATH", mods.to_str().expect("utf-8 path")),
        ],
    )
    .expect("run tool");
    assert_eq!(out.status.code(), Some(255), "{}", out.combined());
}

const UNQUALIFIED: &str = "import QtQuick\nItem {\n    property string s: foo\n}\n";

#[test]
fn qmllint_settings_file_changes_results_only_when_declared() {
    let (_file_dir, file) = scratch_file("Settings.qml", UNQUALIFIED);
    std::fs::write(
        file.with_file_name(".qmllint.ini"),
        "[Warnings]\nUnqualifiedAccess=disable\n",
    )
    .expect("write settings");

    let (_, ignored) = lint(&[file.as_path()], &[], false);
    assert_eq!(ignored.expect("parsed").len(), 1);

    let (out, honored) = lint(&[file.as_path()], &[], true);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(honored.expect("parsed").is_empty());
}

#[test]
fn qmllint_columns_count_utf16_units() {
    let line = "Item { property string s: \"\u{1F600}\"; property int q: foo }";
    let (_file_dir, file) = scratch_file("Emoji.qml", &format!("import QtQuick\n{line}\n"));
    let (_, parsed) = lint(&[file.as_path()], &[], false);
    let found = parsed.expect("parsed");
    assert_eq!(found.len(), 1);
    let finding = &found[0].finding;
    let before = &line[..line.find("foo").expect("foo")];
    let utf16 = before.encode_utf16().count() as u64 + 1;
    assert_ne!(utf16, before.chars().count() as u64 + 1);
    assert_eq!(finding.encoding, quality_adapter::ColumnEncoding::Utf16);
    assert_eq!(finding.start.column, utf16);
}

#[test]
fn qmllint_severity_flags_decide_the_exit_code() {
    let (_file_dir, file) = scratch_file("Severity.qml", UNQUALIFIED);
    let invocation = qmllint_check(&path("DX_QMLLINT"), &[file.as_path()], &[], false);
    let mut argv: Vec<String> = invocation
        .argv
        .iter()
        .map(|arg| arg.to_str().expect("utf-8 argument").to_owned())
        .collect();
    let at = argv.len() - 1;
    argv.splice(at..at, ["--unqualified".to_owned(), "info".to_owned()]);
    let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
    let out = dx_testing::run(Path::new(&argv[0]), &args, &[]).expect("run tool");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let found = parse_qmllint(
        out.stdout.as_bytes(),
        out.status.code(),
        &[file.to_str().expect("utf-8 path")],
    )
    .expect("parsed");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].finding.severity,
        quality_adapter::ToolSeverity::Info
    );
}
