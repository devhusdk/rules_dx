use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct PylintMessage {
    #[serde(rename = "type")]
    kind: String,
    symbol: String,
    message: String,
    #[serde(rename = "message-id")]
    message_id: String,
    line: Option<u64>,
    column: Option<u64>,
    #[serde(rename = "endLine")]
    end_line: Option<u64>,
    #[serde(rename = "endColumn")]
    end_column: Option<u64>,
    path: String,
}

fn pylint_severity(kind: &str) -> Result<ToolSeverity, ParseError> {
    const TOOL: &str = "pylint";
    match kind {
        "fatal" | "error" => Ok(ToolSeverity::Error),
        "warning" | "refactor" | "convention" => Ok(ToolSeverity::Warning),
        "info" | "information" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unknown message type: {kind:?}"),
        }),
    }
}

pub fn parse_pylint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "pylint";
    check_output_size(TOOL, stdout)?;
    let messages: Vec<PylintMessage> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::with_capacity(messages.len());
    for message in &messages {
        let checked = known(TOOL, files, &message.path)?;
        let line = message.line.unwrap_or(0);
        if line < 1 {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("bad line in message {:?}", message.message_id),
            });
        }
        let column = message.column.unwrap_or(0) + 1;
        let start = TextPosition { line, column };
        let end = match (message.end_line, message.end_column) {
            (Some(end_line), Some(end_column)) if end_line >= 1 => Some(TextPosition {
                line: end_line,
                column: end_column + 1,
            }),
            (None, None) => None,
            _ => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("bad end in message {:?}", message.message_id),
                });
            }
        };
        if message.symbol.is_empty() || message.message_id.is_empty() || message.message.is_empty()
        {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: "message with an empty symbol, id, or text".to_owned(),
            });
        }
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: message.message_id.clone(),
                message: message.message.clone(),
                severity: pylint_severity(&message.kind)?,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
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

    const PYLINT_DIRTY: &str = r#"[
    {
        "type": "warning",
        "module": "dirty",
        "obj": "",
        "line": 3,
        "column": 0,
        "endLine": 3,
        "endColumn": 9,
        "path": "/s/dirty.py",
        "symbol": "unused-import",
        "message": "Unused import os",
        "message-id": "W0611"
    }
]"#;

    #[test]
    fn pylint_reports_one_based_ranges() {
        let findings =
            parse_pylint(PYLINT_DIRTY.as_bytes(), Some(4), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.py");
        assert_eq!(findings[0].finding.tool_id, "pylint");
        assert_eq!(findings[0].finding.rule_id, "W0611");
        assert_eq!(findings[0].finding.message, "Unused import os");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 3, column: 1 },
                Some(TextPosition {
                    line: 3,
                    column: 10
                })
            )
        );
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_pylint(b"[]", Some(0), &["/s/dirty.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_pylint(b"[]", Some(4), &["/s/dirty.py"]).is_err());
        assert!(parse_pylint(PYLINT_DIRTY.as_bytes(), Some(4), &["/s/other.py"]).is_err());
        assert!(parse_pylint(b"not json", Some(4), &["/s/dirty.py"]).is_err());
    }

    #[test]
    fn pylint_maps_kinds_and_null_ends() {
        let stdout = r#"[
    {
        "type": "convention",
        "module": "a",
        "obj": "",
        "line": 1,
        "column": 0,
        "endLine": null,
        "endColumn": null,
        "path": "/s/a.py",
        "symbol": "missing-module-docstring",
        "message": "Missing module docstring",
        "message-id": "C0114"
    }
]"#;
        let findings = parse_pylint(stdout.as_bytes(), Some(16), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "C0114");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 1, column: 1 }, None)
        );
        for (kind, severity) in [
            ("fatal", ToolSeverity::Error),
            ("error", ToolSeverity::Error),
            ("refactor", ToolSeverity::Warning),
            ("info", ToolSeverity::Info),
        ] {
            let stdout = format!(
                r#"[{{"type": "{kind}", "module": "a", "obj": "", "line": 2, "column": 4, "endLine": 2, "endColumn": 5, "path": "/s/a.py", "symbol": "sym", "message": "msg", "message-id": "X0001"}}]"#
            );
            let findings = parse_pylint(stdout.as_bytes(), Some(2), &["/s/a.py"]).expect("parsed");
            assert_eq!(findings[0].finding.severity, severity, "kind {kind}");
            assert_eq!(
                (findings[0].finding.start, findings[0].finding.end),
                (
                    TextPosition { line: 2, column: 5 },
                    Some(TextPosition { line: 2, column: 6 })
                )
            );
        }
        let bad_kind = r#"[{"type": "nope", "module": "a", "obj": "", "line": 1, "column": 0, "endLine": null, "endColumn": null, "path": "/s/a.py", "symbol": "sym", "message": "msg", "message-id": "X0001"}]"#;
        assert!(parse_pylint(bad_kind.as_bytes(), Some(1), &["/s/a.py"]).is_err());
        let bad_line = r#"[{"type": "warning", "module": "a", "obj": "", "line": 0, "column": 0, "endLine": null, "endColumn": null, "path": "/s/a.py", "symbol": "sym", "message": "msg", "message-id": "X0001"}]"#;
        assert!(parse_pylint(bad_line.as_bytes(), Some(1), &["/s/a.py"]).is_err());
    }

    #[test]
    fn pylint_grammar_mismatches_are_fail_closed() {
        let bad_end = r#"[{"type": "warning", "module": "a", "obj": "", "line": 1, "column": 0, "endLine": 2, "endColumn": null, "path": "/s/a.py", "symbol": "sym", "message": "msg", "message-id": "X0001"}]"#;
        assert!(parse_pylint(bad_end.as_bytes(), Some(1), &["/s/a.py"]).is_err());
        let empty_symbol = r#"[{"type": "warning", "module": "a", "obj": "", "line": 1, "column": 0, "endLine": null, "endColumn": null, "path": "/s/a.py", "symbol": "", "message": "msg", "message-id": "X0001"}]"#;
        assert!(parse_pylint(empty_symbol.as_bytes(), Some(1), &["/s/a.py"]).is_err());
    }
}
