use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "qmllint";
const REVISION: u64 = 4;
const FAILED: i32 = 255;

#[derive(Debug, Deserialize)]
struct QmlLintReport {
    revision: u64,
    files: Vec<QmlLintFile>,
}

#[derive(Debug, Deserialize)]
struct QmlLintFile {
    filename: String,
    success: bool,
    warnings: Vec<QmlLintRecord>,
}

#[derive(Debug, Deserialize)]
struct QmlLintRecord {
    line: Option<u64>,
    column: Option<u64>,
    id: String,
    message: String,
    #[serde(rename = "type")]
    severity: String,
}

fn qmllint_severity(level: &str) -> Result<ToolSeverity, ParseError> {
    match level {
        "error" | "critical" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        "info" | "note" | "none" => Ok(ToolSeverity::Info),
        other => Err(ParseError::shape(
            TOOL,
            format!("unknown severity: {other}"),
        )),
    }
}

fn read_report(stdout: &[u8]) -> Result<QmlLintReport, ParseError> {
    check_output_size(TOOL, stdout)?;
    let text =
        std::str::from_utf8(stdout).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let report: QmlLintReport = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if report.revision != REVISION {
        return Err(ParseError::shape(
            TOOL,
            format!("unsupported report revision {}", report.revision),
        ));
    }
    Ok(report)
}

/// The exit code a report implies: 255 when any file failed, else 0.
fn implied_code(report: &QmlLintReport) -> i32 {
    if report.files.iter().any(|file| !file.success) {
        FAILED
    } else {
        0
    }
}

fn convert(
    report: &QmlLintReport,
    files: &[&str],
    require_all: bool,
) -> Result<Vec<FileFinding>, ParseError> {
    let mut seen: Vec<&str> = Vec::new();
    let mut findings = Vec::new();
    for file in &report.files {
        let normalized = file.filename.strip_prefix("./").unwrap_or(&file.filename);
        let checked = known(TOOL, files, normalized)?;
        if seen.contains(&checked) {
            return Err(ParseError::shape(
                TOOL,
                format!("{} is reported twice", file.filename),
            ));
        }
        seen.push(checked);
        let mut failing = false;
        for record in &file.warnings {
            let severity = qmllint_severity(&record.severity)?;
            failing |= severity != ToolSeverity::Info;
            let (Some(line), Some(column)) = (record.line, record.column) else {
                return Err(ParseError::shape(
                    TOOL,
                    format!("{}: {} [{}]", file.filename, record.message, record.id),
                ));
            };
            if record.id.is_empty() || line == 0 || column == 0 {
                return Err(ParseError::shape(
                    TOOL,
                    format!("malformed record for {}", file.filename),
                ));
            }
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: record.id.clone(),
                    message: record.message.clone(),
                    severity,
                    start: TextPosition { line, column },
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf16,
                    related: Vec::new(),
                },
            });
        }
        if failing == file.success {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "{} reports success {} with {} diagnostics",
                    file.filename,
                    file.success,
                    file.warnings.len()
                ),
            ));
        }
    }
    if require_all {
        if let Some(missing) = files.iter().find(|file| !seen.contains(file)) {
            return Err(ParseError::shape(TOOL, format!("no report for {missing}")));
        }
    }
    Ok(findings)
}

pub fn parse_qmllint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with empty output", code_name(code)),
        ));
    }
    let report = read_report(stdout)?;
    let findings = convert(&report, files, true)?;
    let want = implied_code(&report);
    if code != Some(want) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "{} diagnostics with exit {} (want {want})",
                findings.len(),
                code_name(code)
            ),
        ));
    }
    Ok(findings)
}

/// Parses one saved report; it may cover only some of the files.
pub fn parse_qmllint_report(stdout: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let report = read_report(stdout)?;
    convert(&report, files, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRITICAL: &str = r#"{"files":[{"filename":"warn.qml","success":false,"warnings":[{"charOffset":49,"column":28,"id":"unqualified","length":3,"line":3,"message":"Unqualified access","suggestions":[{"charOffset":49,"column":28,"hint":"","isAutoApplicable":false,"length":3,"line":3,"message":"Did you mean \"top\"?","replacement":"top"}],"type":"critical"}]}],"revision":4}"#;
    const INFO: &str = r#"{"files":[{"filename":"warn.qml","success":true,"warnings":[{"charOffset":49,"column":28,"id":"unqualified","length":3,"line":3,"message":"Unqualified access","suggestions":[{"charOffset":49,"column":28,"hint":"","isAutoApplicable":false,"length":3,"line":3,"message":"Did you mean \"top\"?","replacement":"top"}],"type":"info"}]}],"revision":4}"#;
    const SYNTAX: &str = r#"{"files":[{"filename":"syntax.qml","success":false,"warnings":[{"charOffset":48,"column":1,"id":"syntax","length":1,"line":4,"message":"Expected token `numeric literal'","suggestions":[],"type":"warning"}]}],"revision":4}"#;
    const IMPORT: &str = r#"{"files":[{"filename":"imp.qml","success":false,"warnings":[{"charOffset":0,"column":1,"id":"import","length":6,"line":1,"message":"Warnings occurred while importing module \"Nope.Missing\":","suggestions":[],"type":"warning"},{"charOffset":0,"column":1,"id":"import","length":6,"line":1,"message":"Failed to import Nope.Missing. Are your import paths set up properly?","suggestions":[],"type":"warning"},{"charOffset":20,"column":1,"id":"import","length":4,"line":2,"message":"Item was not found. Did you add all imports and dependencies?","suggestions":[],"type":"warning"}]}],"revision":4}"#;
    const MISSING_FILE: &str = r#"{"files":[{"filename":"nonexist.qml","success":false,"warnings":[{"id":"import","message":"Failed to open file nonexist.qml: No such file or directory","suggestions":[],"type":"warning"}]}],"revision":4}"#;
    const EMOJI: &str = r#"{"files":[{"filename":"emoji.qml","success":false,"warnings":[{"charOffset":64,"column":50,"id":"unqualified","length":3,"line":2,"message":"Unqualified access","suggestions":[{"charOffset":64,"column":50,"hint":"","isAutoApplicable":false,"length":3,"line":2,"message":"Did you mean \"top\"?","replacement":"top"}],"type":"warning"}]}],"revision":4}"#;
    const CLEAN: &str =
        r#"{"files":[{"filename":"clean.qml","success":true,"warnings":[]}],"revision":4}"#;

    fn warning() -> String {
        CRITICAL.replace("\"critical\"", "\"warning\"")
    }

    #[test]
    fn warnings_fail_the_run_and_keep_their_location() {
        let findings =
            parse_qmllint(warning().as_bytes(), Some(255), &["warn.qml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0].finding;
        assert_eq!(findings[0].file, "warn.qml");
        assert_eq!(finding.rule_id, "unqualified");
        assert_eq!(finding.message, "Unqualified access");
        assert_eq!(finding.severity, ToolSeverity::Warning);
        assert_eq!((finding.start.line, finding.start.column), (3, 28));
        assert_eq!(finding.encoding, ColumnEncoding::Utf16);
        assert!(parse_qmllint(warning().as_bytes(), Some(0), &["warn.qml"]).is_err());
        assert!(parse_qmllint(warning().as_bytes(), Some(1), &["warn.qml"]).is_err());
    }

    #[test]
    fn critical_records_are_errors() {
        let findings =
            parse_qmllint(CRITICAL.as_bytes(), Some(255), &["warn.qml"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn info_records_are_reported_and_exit_zero() {
        let findings = parse_qmllint(INFO.as_bytes(), Some(0), &["warn.qml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert!(parse_qmllint(INFO.as_bytes(), Some(255), &["warn.qml"]).is_err());
    }

    #[test]
    fn clean_reports_need_exit_zero() {
        let clean = parse_qmllint(CLEAN.as_bytes(), Some(0), &["clean.qml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_qmllint(CLEAN.as_bytes(), Some(255), &["clean.qml"]).is_err());
    }

    #[test]
    fn syntax_and_import_failures_are_findings() {
        let syntax = parse_qmllint(SYNTAX.as_bytes(), Some(255), &["syntax.qml"]).expect("parsed");
        assert_eq!(syntax[0].finding.rule_id, "syntax");
        assert_eq!(
            (syntax[0].finding.start.line, syntax[0].finding.start.column),
            (4, 1)
        );
        let import = parse_qmllint(IMPORT.as_bytes(), Some(255), &["imp.qml"]).expect("parsed");
        assert_eq!(import.len(), 3);
        assert!(import.iter().all(|found| found.finding.rule_id == "import"));
        assert!(import[1]
            .finding
            .message
            .contains("Failed to import Nope.Missing"));
    }

    #[test]
    fn utf16_columns_count_surrogate_pairs() {
        let findings = parse_qmllint(EMOJI.as_bytes(), Some(255), &["emoji.qml"]).expect("parsed");
        assert_eq!(findings[0].finding.start.column, 50);
        assert_eq!(findings[0].finding.encoding, ColumnEncoding::Utf16);
    }

    #[test]
    fn a_record_without_a_location_is_an_infrastructure_failure() {
        let err = parse_qmllint(MISSING_FILE.as_bytes(), Some(255), &["nonexist.qml"])
            .expect_err("no location");
        assert!(
            err.to_string().contains("Failed to open file nonexist.qml"),
            "{err}"
        );
    }

    #[test]
    fn every_requested_file_must_be_reported_once() {
        let missing = parse_qmllint(CLEAN.as_bytes(), Some(0), &["clean.qml", "other.qml"]);
        assert!(missing
            .expect_err("missing")
            .to_string()
            .contains("no report for other.qml"));
        let twice = CLEAN.replace(
            "]",
            ",{\"filename\":\"clean.qml\",\"success\":true,\"warnings\":[]}]",
        );
        assert!(parse_qmllint(twice.as_bytes(), Some(0), &["clean.qml"]).is_err());
        assert!(parse_qmllint(CLEAN.as_bytes(), Some(0), &["other.qml"]).is_err());
    }

    #[test]
    fn unsupported_or_malformed_reports_are_never_clean() {
        let old = CLEAN.replace("\"revision\":4", "\"revision\":3");
        assert!(parse_qmllint(old.as_bytes(), Some(0), &["clean.qml"]).is_err());
        let diagnostics_only = r#"{"diagnostics":[],"revision":4}"#;
        assert!(parse_qmllint(diagnostics_only.as_bytes(), Some(0), &["clean.qml"]).is_err());
        assert!(parse_qmllint(b"", Some(0), &["clean.qml"]).is_err());
        assert!(parse_qmllint(b"", Some(255), &["clean.qml"]).is_err());
        assert!(parse_qmllint(b"not json", Some(255), &["x"]).is_err());
        assert!(parse_qmllint(&[0xff], Some(255), &["x"]).is_err());
        let zero = warning().replace("\"line\":3", "\"line\":0");
        assert!(parse_qmllint(zero.as_bytes(), Some(255), &["warn.qml"]).is_err());
        let unknown = warning().replace("\"warning\"", "\"loud\"");
        assert!(parse_qmllint(unknown.as_bytes(), Some(255), &["warn.qml"]).is_err());
        let lying = warning().replace("\"success\":false", "\"success\":true");
        assert!(parse_qmllint(lying.as_bytes(), Some(0), &["warn.qml"]).is_err());
    }

    #[test]
    fn saved_reports_derive_their_exit_from_the_report() {
        let info = parse_qmllint_report(INFO.as_bytes(), &["warn.qml"]).expect("parsed");
        assert_eq!(info.len(), 1);
        let warned = parse_qmllint_report(warning().as_bytes(), &["warn.qml"]).expect("parsed");
        assert_eq!(warned.len(), 1);
        assert!(parse_qmllint_report(b"", &["warn.qml"]).is_err());
        let partial = parse_qmllint_report(CLEAN.as_bytes(), &["clean.qml", "other.qml"]);
        assert!(partial.expect("one report per source").is_empty());
    }
}
