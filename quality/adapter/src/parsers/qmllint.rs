use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct QmlLintReport {
    revision: u64,
    files: Vec<QmlLintFile>,
}

#[derive(Debug, Deserialize)]
struct QmlLintFile {
    filename: String,
    #[serde(default)]
    warnings: Vec<QmlLintRecord>,
}

#[derive(Debug, Deserialize)]
struct QmlLintRecord {
    line: u64,
    column: u64,
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
        other => Err(ParseError::Shape {
            tool: "qmllint",
            detail: format!("unknown severity: {other}"),
        }),
    }
}

pub fn parse_qmllint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "qmllint";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if text.trim().is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with empty output", code_name(code)),
        });
    }
    let report: QmlLintReport = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if report.revision != 4 {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unsupported report revision {}", report.revision),
        });
    }
    let mut findings = Vec::new();
    for file in &report.files {
        let normalized = file.filename.strip_prefix("./").unwrap_or(&file.filename);
        let checked = known(TOOL, files, normalized)?;
        for record in &file.warnings {
            if record.id.is_empty() || record.line == 0 || record.column == 0 {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("malformed record for {}", file.filename),
                });
            }
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: record.id.clone(),
                    message: record.message.clone(),
                    severity: qmllint_severity(&record.severity)?,
                    start: TextPosition {
                        line: record.line,
                        column: record.column,
                    },
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
        }
    }
    let want = if findings.is_empty() { 0 } else { 255 };
    if code != Some(want) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "{} diagnostics with exit {} (want {want})",
                findings.len(),
                code_name(code)
            ),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"files":[{"filename":"qml/Main.qml","success":false,"warnings":[{"charOffset":50,"column":28,"id":"unqualified","length":3,"line":4,"message":"Unqualified access","suggestions":[],"type":"warning"}]}],"revision":4}"#;
    const CLEAN: &str =
        r#"{"files":[{"filename":"qml/Main.qml","success":true,"warnings":[]}],"revision":4}"#;

    #[test]
    fn qmllint_reports_json_warnings() {
        let findings =
            parse_qmllint(DIRTY.as_bytes(), Some(255), &["qml/Main.qml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "unqualified");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.column, 28);
        let clean = parse_qmllint(CLEAN.as_bytes(), Some(0), &["qml/Main.qml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_qmllint(b"", Some(255), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(DIRTY.as_bytes(), Some(0), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(DIRTY.as_bytes(), Some(1), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(CLEAN.as_bytes(), Some(255), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(DIRTY.as_bytes(), Some(255), &["other.qml"]).is_err());
        assert!(parse_qmllint(b"not json", Some(255), &["x"]).is_err());
        assert!(parse_qmllint(&[0xff], Some(255), &["x"]).is_err());
        let old = DIRTY.replace("\"revision\":4", "\"revision\":3");
        assert!(parse_qmllint(old.as_bytes(), Some(255), &["qml/Main.qml"]).is_err());
        let zero = DIRTY.replace("\"line\":4", "\"line\":0");
        assert!(parse_qmllint(zero.as_bytes(), Some(255), &["qml/Main.qml"]).is_err());
        let unknown = DIRTY.replace("\"warning\"}", "\"loud\"}");
        assert!(parse_qmllint(unknown.as_bytes(), Some(255), &["qml/Main.qml"]).is_err());
    }
}
