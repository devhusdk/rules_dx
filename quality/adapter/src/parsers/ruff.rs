use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct RuffPosition {
    column: u64,
    row: u64,
}

#[derive(Debug, Deserialize)]
struct RuffDiagnostic {
    code: String,
    filename: String,
    location: RuffPosition,
    end_location: RuffPosition,
    message: String,
    severity: String,
}

fn ruff_severity(tool: &'static str, severity: &str) -> Result<ToolSeverity, ParseError> {
    match severity {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        _ => Err(ParseError::Shape {
            tool,
            detail: format!("unknown severity: {severity}"),
        }),
    }
}

fn ruff_position(
    tool: &'static str,
    what: &str,
    position: &RuffPosition,
) -> Result<TextPosition, ParseError> {
    if position.row == 0 || position.column == 0 {
        return Err(ParseError::Shape {
            tool,
            detail: format!("zero {what} position"),
        });
    }
    Ok(TextPosition {
        line: position.row,
        column: position.column,
    })
}

pub fn parse_ruff(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "ruff";
    check_output_size(TOOL, stdout)?;
    let diagnostics: Vec<RuffDiagnostic> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        let checked = known(TOOL, files, &diagnostic.filename)?;
        let start = ruff_position(TOOL, "start", &diagnostic.location)?;
        let end = ruff_position(TOOL, "end", &diagnostic.end_location)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                severity: ruff_severity(TOOL, &diagnostic.severity)?,
                start,
                end: Some(end),
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty findings array", code_name(code)),
        });
    }
    Ok(findings)
}

pub fn parse_ruff_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "ruff_format";
    check_output_size(TOOL, stdout)?;
    let diagnostics: Vec<RuffDiagnostic> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        if diagnostic.code != "unformatted" {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unexpected code {:?}", diagnostic.code),
            });
        }
        let checked = known(TOOL, files, &diagnostic.filename)?;
        let start = ruff_position(TOOL, "start", &diagnostic.location)?;
        let end = ruff_position(TOOL, "end", &diagnostic.end_location)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: "ruff".to_owned(),
                rule_id: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                severity: ruff_severity(TOOL, &diagnostic.severity)?,
                start,
                end: Some(end),
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty findings array", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUFF_LINT_DIRTY: &str = r#"[{"cell":null,"code":"F401","end_location":{"column":10,"row":1},"filename":"/s/dirty.py","fix":{"applicability":"safe","edits":[{"content":"","end_location":{"column":1,"row":2},"location":{"column":1,"row":1}}],"message":"Remove unused import: `os`"},"location":{"column":8,"row":1},"message":"`os` imported but unused","name":"unused-import","noqa_row":1,"severity":"error","url":"https://docs.astral.sh/ruff/rules/unused-import"}]"#;

    const RUFF_FORMAT_DIRTY: &str = r#"[{"cell":null,"code":"unformatted","end_location":{"column":7,"row":5},"filename":"/s/dirty.py","fix":{"applicability":"safe","edits":[{"content":" = ","end_location":{"column":7,"row":5},"location":{"column":6,"row":5}}],"message":null},"location":{"column":6,"row":5},"message":"File would be reformatted","name":"unformatted","noqa_row":null,"severity":"error","url":null}]"#;

    #[test]
    fn ruff_reports_lint_diagnostics() {
        let findings =
            parse_ruff(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.py");
        assert_eq!(findings[0].finding.tool_id, "ruff");
        assert_eq!(findings[0].finding.rule_id, "F401");
        assert_eq!(findings[0].finding.message, "`os` imported but unused");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 1, column: 8 },
                Some(TextPosition {
                    line: 1,
                    column: 10
                })
            )
        );
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_ruff(b"[]", Some(0), &["/s/dirty.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_ruff(b"[]", Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_ruff(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/other.py"]).is_err());
        assert!(parse_ruff(b"not json", Some(1), &["/s/dirty.py"]).is_err());
    }

    #[test]
    fn ruff_format_accepts_only_unformatted() {
        let findings = parse_ruff_format(RUFF_FORMAT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.tool_id, "ruff");
        assert_eq!(findings[0].finding.rule_id, "unformatted");
        assert_eq!(findings[0].finding.message, "File would be reformatted");
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 5, column: 6 },
                Some(TextPosition { line: 5, column: 7 })
            )
        );
        assert!(parse_ruff_format(b"[]", Some(0), &["/s/dirty.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_ruff_format(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"]).is_err());
    }

    #[test]
    fn ruff_grammar_mismatches_are_fail_closed() {
        let warning = RUFF_LINT_DIRTY.replace("\"severity\":\"error\"", "\"severity\":\"warning\"");
        let findings = parse_ruff(warning.as_bytes(), Some(1), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let unknown = RUFF_LINT_DIRTY.replace("\"severity\":\"error\"", "\"severity\":\"info\"");
        assert!(parse_ruff(unknown.as_bytes(), Some(1), &["/s/dirty.py"]).is_err());
        let zero_row = RUFF_LINT_DIRTY.replace(
            "\"location\":{\"column\":8,\"row\":1}",
            "\"location\":{\"column\":8,\"row\":0}",
        );
        assert!(parse_ruff(zero_row.as_bytes(), Some(1), &["/s/dirty.py"]).is_err());
        let zero_col = RUFF_LINT_DIRTY.replace(
            "\"location\":{\"column\":8,\"row\":1}",
            "\"location\":{\"column\":0,\"row\":1}",
        );
        assert!(parse_ruff(zero_col.as_bytes(), Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_ruff_format(b"not json", Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_ruff_format(b"[]", Some(1), &["/s/dirty.py"]).is_err());
    }
}
