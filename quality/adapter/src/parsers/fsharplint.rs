use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct FSharpLintRecord {
    path: String,
    #[serde(default)]
    severity: Option<String>,
    rule: String,
    message: String,
    #[serde(rename = "startLine")]
    start_line: u64,
    #[serde(rename = "startColumn")]
    start_column: u64,
    #[serde(rename = "endLine")]
    end_line: u64,
    #[serde(rename = "endColumn")]
    end_column: u64,
}

pub fn parse_fsharplint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "fsharplint";
    check_output_size(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0)", code_name(code)),
        });
    }
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: FSharpLintRecord =
            serde_json::from_str(trimmed).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
        if record.rule.is_empty()
            || record.start_line == 0
            || record.start_column == 0
            || record.end_line == 0
            || record.end_column == 0
        {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("malformed record: {trimmed}"),
            });
        }
        let severity = match record.severity.as_deref() {
            None | Some("warning") => ToolSeverity::Warning,
            Some("error") => ToolSeverity::Error,
            Some(other) => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("unknown severity: {other}"),
                });
            }
        };
        let checked = known(TOOL, files, &record.path)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: record.rule,
                message: record.message,
                severity,
                start: TextPosition {
                    line: record.start_line,
                    column: record.start_column,
                },
                end: Some(TextPosition {
                    line: record.end_line,
                    column: record.end_column,
                }),
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINT: &str = "{\"path\": \"/s/Sample.fs\", \"rule\": \"FL0036\", \"message\": \"Consider changing `ExampleInterface` to be prefixed with `I`.\", \"startLine\": 3, \"startColumn\": 6, \"endLine\": 3, \"endColumn\": 23}\n{\"path\": \"/s/Sample.fs\", \"rule\": \"FL0034\", \"message\": \"If `( + )` has no mutable arguments partially applied then the lambda can be removed.\", \"startLine\": 6, \"startColumn\": 23, \"endLine\": 6, \"endColumn\": 36}\n";

    #[test]
    fn fsharplint_reports_library_records() {
        let findings =
            parse_fsharplint(LINT.as_bytes(), Some(0), &["/s/Sample.fs"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.rule_id, "FL0036");
        assert_eq!(findings[1].finding.rule_id, "FL0034");
        let clean = parse_fsharplint(b"", Some(0), &["/s/Sample.fs"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_fsharplint(LINT.as_bytes(), Some(1), &["/s/Sample.fs"]).is_err());
        assert!(parse_fsharplint(LINT.as_bytes(), Some(0), &["/s/other.fs"]).is_err());
        let syntax = "{\"path\": \"/s/Sample.fs\", \"severity\": \"error\", \"rule\": \"FS0010\", \"message\": \"Unexpected symbol\", \"startLine\": 2, \"startColumn\": 9, \"endLine\": 2, \"endColumn\": 10}\n";
        let found =
            parse_fsharplint(syntax.as_bytes(), Some(0), &["/s/Sample.fs"]).expect("parsed");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.rule_id, "FS0010");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let odd = syntax.replace("error", "fatal");
        assert!(parse_fsharplint(odd.as_bytes(), Some(0), &["/s/Sample.fs"]).is_err());
        assert!(parse_fsharplint(b"not json\n", Some(0), &["/s/Sample.fs"]).is_err());
        assert!(parse_fsharplint(&[0xff], Some(0), &["/s/Sample.fs"]).is_err());
    }
}
