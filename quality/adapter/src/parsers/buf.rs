use serde::Deserialize;

use super::{
    as_text, check_output_size, code_name, known, unformatted_at, FileFinding, ParseError,
};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const FINDINGS_EXIT: i32 = 100;

#[derive(Debug, Deserialize)]
struct BufLintRecord {
    path: String,
    #[serde(default)]
    start_line: Option<u64>,
    #[serde(default)]
    start_column: Option<u64>,
    #[serde(default)]
    end_line: Option<u64>,
    #[serde(default)]
    end_column: Option<u64>,
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

pub fn parse_buf_lint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "buf";
    check_output_size(TOOL, stdout)?;
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
        let record: BufLintRecord =
            serde_json::from_str(trimmed).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
        if record.kind.is_empty() || record.message.is_empty() {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("malformed record: {trimmed}"),
            });
        }
        let (start_line, start_column) = match (record.start_line, record.start_column) {
            (Some(line), Some(column)) => {
                if line == 0 || column == 0 {
                    return Err(ParseError::Shape {
                        tool: TOOL,
                        detail: format!("malformed start: {trimmed}"),
                    });
                }
                (line, column)
            }
            _ => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("missing start: {trimmed}"),
                });
            }
        };
        let end = match (record.end_line, record.end_column) {
            (Some(line), Some(column)) => {
                if line == 0 || column == 0 {
                    return Err(ParseError::Shape {
                        tool: TOOL,
                        detail: format!("malformed end: {trimmed}"),
                    });
                }
                Some(TextPosition { line, column })
            }
            (None, None) => None,
            _ => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("partial end: {trimmed}"),
                });
            }
        };
        let checked = known(TOOL, files, &record.path)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: record.kind,
                message: record.message,
                severity: ToolSeverity::Error,
                start: TextPosition {
                    line: start_line,
                    column: start_column,
                },
                end,
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
                detail: format!("exit {} with no findings", code_name(code)),
            });
        }
    } else if code != Some(FINDINGS_EXIT) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "findings with exit {} (want {FINDINGS_EXIT})",
                code_name(code)
            ),
        });
    }
    Ok(findings)
}

pub fn parse_buf_format(
    stdout: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "buf";
    as_text(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0)", code_name(code)),
        });
    }
    if stdout == input {
        return Ok(Vec::new());
    }
    Ok(vec![unformatted_at(TOOL, file, 1, 1)])
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINT: &str = "{\"path\": \"proto/hello.proto\", \"start_line\": 4, \"start_column\": 3, \"end_line\": 4, \"end_column\": 12, \"type\": \"PACKAGE_DIRECTORY_MATCH\", \"message\": \"Files with package foo must be in a directory foo.\"}\n";

    #[test]
    fn buf_lint_reports_jsonl_records() {
        let findings =
            parse_buf_lint(LINT.as_bytes(), Some(100), &["proto/hello.proto"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "PACKAGE_DIRECTORY_MATCH");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        let clean = parse_buf_lint(b"", Some(0), &["proto/hello.proto"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_buf_lint(LINT.as_bytes(), Some(0), &["proto/hello.proto"]).is_err());
        assert!(parse_buf_lint(b"", Some(1), &["proto/hello.proto"]).is_err());
        assert!(parse_buf_lint(LINT.as_bytes(), Some(1), &["proto/hello.proto"]).is_err());
        assert!(parse_buf_lint(LINT.as_bytes(), Some(100), &["other.proto"]).is_err());
        assert!(parse_buf_lint(b"not json\n", Some(100), &["proto/hello.proto"]).is_err());
        assert!(parse_buf_lint(&[0xff], Some(100), &["x"]).is_err());
    }

    #[test]
    fn buf_format_reports_a_file_whose_formatted_output_differs() {
        let formatted = b"syntax = \"proto3\";\n";
        let findings = parse_buf_format(formatted, Some(0), b"syntax=\"proto3\";\n", "a.proto")
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_buf_format(formatted, Some(0), formatted, "a.proto").expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_buf_format(formatted, Some(1), formatted, "a.proto").is_err());
        assert!(parse_buf_format(formatted, None, formatted, "a.proto").is_err());
        assert!(parse_buf_format(&[0xff], Some(0), b"", "a.proto").is_err());
    }
}
