use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct QmlLintReport {
    #[serde(default)]
    diagnostics: Vec<QmlLintRecord>,
}

#[derive(Debug, Deserialize)]
struct QmlLintRecord {
    file: String,
    line: u64,
    #[serde(default = "default_column")]
    column: u64,
    #[serde(default)]
    rule: String,
    message: String,
    #[serde(default = "default_severity")]
    severity: String,
}

fn default_column() -> u64 {
    1
}

fn default_severity() -> String {
    "warning".to_owned()
}

fn qmllint_severity(level: &str) -> Result<ToolSeverity, ParseError> {
    match level {
        "error" => Ok(ToolSeverity::Error),
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
    let mut findings = Vec::with_capacity(report.diagnostics.len());
    for record in &report.diagnostics {
        if record.rule.is_empty() || record.line == 0 || record.column == 0 {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("malformed record for {}", record.file),
            });
        }
        let normalized = record.file.strip_prefix("./").unwrap_or(&record.file);
        let checked = known(TOOL, files, normalized)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: record.rule.clone(),
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
    if findings.is_empty() {
        if code != Some(0) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("exit {} with no diagnostics", code_name(code)),
            });
        }
    } else if code != Some(1) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("diagnostics with exit {} (want 1)", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"diagnostics": [{"file": "qml/Main.qml", "line": 4, "column": 5, "rule": "unqualified", "message": "Unqualified access to `foo`.", "severity": "warning"}]}"#;
    const CLEAN: &str = r#"{"diagnostics": []}"#;

    #[test]
    fn qmllint_reports_json_diagnostics() {
        let findings = parse_qmllint(DIRTY.as_bytes(), Some(1), &["qml/Main.qml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "unqualified");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let clean = parse_qmllint(CLEAN.as_bytes(), Some(0), &["qml/Main.qml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_qmllint(b"", Some(1), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(DIRTY.as_bytes(), Some(0), &["qml/Main.qml"]).is_err());
        assert!(parse_qmllint(DIRTY.as_bytes(), Some(1), &["other.qml"]).is_err());
        assert!(parse_qmllint(b"not json", Some(1), &["x"]).is_err());
        assert!(parse_qmllint(&[0xff], Some(1), &["x"]).is_err());
    }
}
