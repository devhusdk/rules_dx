use super::{check_output_size, code_name, known, point, unformatted_at, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "psscriptanalyzer";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn position(value: &serde_json::Value, key: &str) -> Result<u64, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|number| number.max(1))
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

pub fn parse_psscriptanalyzer(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let entries = report_entries(stdout, code)?;
    let mut findings = Vec::new();
    let mut reported: Vec<&str> = Vec::new();
    for entry in &entries {
        let checked = known(TOOL, files, text(entry, "path")?)?;
        reported.push(checked);
        let diagnostics = entry
            .get("diagnostics")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ParseError::shape(TOOL, "missing diagnostics"))?;
        for diagnostic in diagnostics {
            let severity = match text(diagnostic, "severity")? {
                "Information" => ToolSeverity::Info,
                "Warning" => ToolSeverity::Warning,
                "Error" | "ParseError" => ToolSeverity::Error,
                other => {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("unknown severity {other:?}"),
                    ))
                }
            };
            let message = text(diagnostic, "message")?;
            if message.is_empty() {
                return Err(ParseError::shape(TOOL, "missing message"));
            }
            let (start, end) = point(
                position(diagnostic, "line")?,
                position(diagnostic, "column")?,
            );
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: text(diagnostic, "rule")?.to_owned(),
                    message: message.to_owned(),
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf16,
                    related: Vec::new(),
                },
            });
        }
    }
    if let Some(missing) = files.iter().find(|file| !reported.contains(file)) {
        return Err(ParseError::shape(TOOL, format!("no result for {missing}")));
    }
    Ok(findings)
}

fn report_entries(stdout: &[u8], code: Option<i32>) -> Result<Vec<serde_json::Value>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::shape(TOOL, format!("exit {}", code_name(code))));
    }
    let mut report: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    if let Some(error) = report.get("error").and_then(serde_json::Value::as_str) {
        return Err(ParseError::shape(TOOL, error));
    }
    match report.get_mut("files").map(serde_json::Value::take) {
        Some(serde_json::Value::Array(entries)) => Ok(entries),
        _ => Err(ParseError::shape(TOOL, "missing files")),
    }
}

/// Parses the `--format-check` report: an unformatted file is one fixable finding, a file that
/// does not parse is an error at each parse failure, and a failed run is never clean.
pub fn parse_psscriptanalyzer_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let entries = report_entries(stdout, code)?;
    let mut findings = Vec::new();
    let mut reported: Vec<&str> = Vec::new();
    for entry in &entries {
        let checked = known(TOOL, files, text(entry, "path")?)?;
        if reported.contains(&checked) {
            return Err(ParseError::shape(TOOL, format!("{checked} reported twice")));
        }
        reported.push(checked);
        if let Some(errors) = entry.get("parse_errors") {
            let errors = errors
                .as_array()
                .filter(|errors| !errors.is_empty())
                .ok_or_else(|| ParseError::shape(TOOL, "empty parse_errors"))?;
            for error in errors {
                let message = text(error, "message")?;
                if message.is_empty() {
                    return Err(ParseError::shape(TOOL, "missing message"));
                }
                let (start, end) = point(position(error, "line")?, position(error, "column")?);
                findings.push(FileFinding {
                    file: checked.to_owned(),
                    finding: Finding {
                        tool_id: TOOL.to_owned(),
                        rule_id: "ParseError".to_owned(),
                        message: message.to_owned(),
                        severity: ToolSeverity::Error,
                        start,
                        end,
                        suggestions: Vec::new(),
                        encoding: ColumnEncoding::Utf16,
                        related: Vec::new(),
                    },
                });
            }
            continue;
        }
        match entry.get("formatted").and_then(serde_json::Value::as_bool) {
            Some(true) => {}
            Some(false) => {
                findings.push(unformatted_at(TOOL, checked, 1, 1, ColumnEncoding::Utf16))
            }
            None => return Err(ParseError::shape(TOOL, "missing formatted")),
        }
    }
    if let Some(missing) = files.iter().find(|file| !reported.contains(file)) {
        return Err(ParseError::shape(TOOL, format!("no result for {missing}")));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = r#"{"analyzer":"1.25.0","files":[{"path":"Sample.ps1","diagnostics":[{"rule":"PSAvoidUsingWriteHost","severity":"Warning","line":4,"column":1,"message":"Avoid using Write-Host"}]}]}"#;
    const CLEAN: &str = r#"{"analyzer":"1.25.0","files":[{"path":"Sample.ps1","diagnostics":[]}]}"#;

    #[test]
    fn psscriptanalyzer_reports_rule_severity_and_position() {
        let findings =
            parse_psscriptanalyzer(DIRTY.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "PSAvoidUsingWriteHost");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.line, 4);
        let clean =
            parse_psscriptanalyzer(CLEAN.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert!(clean.is_empty());
    }

    #[test]
    fn parse_errors_are_errors() {
        let broken = DIRTY.replace("Warning", "ParseError");
        let findings =
            parse_psscriptanalyzer(broken.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn psscriptanalyzer_never_reports_a_failed_run_as_clean() {
        assert!(parse_psscriptanalyzer(b"", Some(0), &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(CLEAN.as_bytes(), Some(1), &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(CLEAN.as_bytes(), None, &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(DIRTY.as_bytes(), Some(0), &["other.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(&[0xff], Some(0), &["x"]).is_err());
        assert!(
            parse_psscriptanalyzer(CLEAN.as_bytes(), Some(0), &["Sample.ps1", "Other.ps1"])
                .is_err()
        );
        let failed = r#"{"error":"IncludeRules names the unknown rule Nope"}"#;
        let error = parse_psscriptanalyzer(failed.as_bytes(), Some(0), &["Sample.ps1"])
            .expect_err("failed");
        assert!(error.to_string().contains("unknown rule Nope"), "{error}");
        let odd = DIRTY.replace("Warning", "Fatal");
        assert!(parse_psscriptanalyzer(odd.as_bytes(), Some(0), &["Sample.ps1"]).is_err());
    }

    const FORMAT: &str = r#"{"analyzer":"1.25.0","files":[{"path":"Bad.ps1","formatted":false},{"path":"Good.ps1","formatted":true},{"path":"Broken.ps1","parse_errors":[{"line":1,"column":12,"message":"Missing closing '}'"}]}]}"#;
    const FORMAT_FILES: &[&str] = &["Bad.ps1", "Good.ps1", "Broken.ps1"];

    #[test]
    fn format_reports_unformatted_files_and_parse_errors() {
        let findings = parse_psscriptanalyzer_format(FORMAT.as_bytes(), Some(0), FORMAT_FILES)
            .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Bad.ps1");
        assert_eq!(findings[0].finding.rule_id, "");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[1].file, "Broken.ps1");
        assert_eq!(findings[1].finding.rule_id, "ParseError");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (
                findings[1].finding.start.line,
                findings[1].finding.start.column
            ),
            (1, 12)
        );
    }

    #[test]
    fn format_never_reports_a_failed_run_as_clean() {
        let parse = |out: &str, code: Option<i32>, files: &[&str]| {
            parse_psscriptanalyzer_format(out.as_bytes(), code, files)
        };
        assert!(parse(FORMAT, Some(1), FORMAT_FILES).is_err());
        assert!(parse(FORMAT, None, FORMAT_FILES).is_err());
        assert!(parse("", Some(0), FORMAT_FILES).is_err());
        assert!(parse(FORMAT, Some(0), &["Bad.ps1", "Good.ps1"]).is_err());
        assert!(parse(
            FORMAT,
            Some(0),
            &["Bad.ps1", "Good.ps1", "Broken.ps1", "More.ps1"]
        )
        .is_err());
        let error = parse(
            r#"{"error":"S.psd1 configures no formatter rule"}"#,
            Some(0),
            &["Bad.ps1"],
        )
        .expect_err("failed");
        assert!(error.to_string().contains("no formatter rule"), "{error}");
        let missing = r#"{"files":[{"path":"Bad.ps1"}]}"#;
        assert!(parse(missing, Some(0), &["Bad.ps1"]).is_err());
        let empty = r#"{"files":[{"path":"Bad.ps1","parse_errors":[]}]}"#;
        assert!(parse(empty, Some(0), &["Bad.ps1"]).is_err());
        let twice = r#"{"files":[{"path":"Bad.ps1","formatted":true},{"path":"Bad.ps1","formatted":true}]}"#;
        assert!(parse(twice, Some(0), &["Bad.ps1"]).is_err());
    }
}
