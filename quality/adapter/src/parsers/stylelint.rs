use super::{check_output_size, code_name, known, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

pub fn parse_stylelint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "stylelint";
    check_output_size(TOOL, stdout)?;
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let entries = value.as_array().ok_or_else(|| ParseError::Shape {
        tool: TOOL,
        detail: "expected JSON array".to_owned(),
    })?;
    let mut findings = Vec::new();
    for entry in entries {
        let source = entry
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: "missing source".to_owned(),
            })?;
        let checked = known(TOOL, files, source)?;
        let warnings = entry
            .get("warnings")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: "missing warnings".to_owned(),
            })?;
        for warn in warnings {
            let line =
                warn.get("line")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ParseError::Shape {
                        tool: TOOL,
                        detail: "missing line".to_owned(),
                    })?;
            let col =
                warn.get("column")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ParseError::Shape {
                        tool: TOOL,
                        detail: "missing column".to_owned(),
                    })?;
            let rule = warn
                .get("rule")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let text = warn
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if text.is_empty() {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: "missing text".to_owned(),
                });
            }
            let severity = match warn.get("severity").and_then(|v| v.as_str()) {
                Some("error") => ToolSeverity::Error,
                Some("warning") => ToolSeverity::Warning,
                other => {
                    return Err(ParseError::Shape {
                        tool: TOOL,
                        detail: format!("unknown severity {other:?}"),
                    })
                }
            };
            if line == 0 || col == 0 {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: "positions must be nonzero".to_owned(),
                });
            }
            let (start, end) = point(line, col);
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: rule,
                    message: text,
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = r#"[{"source": "style.css", "warnings": [{"line": 2, "column": 5, "rule": "color-no-invalid-hex", "text": "Unexpected invalid hex color", "severity": "error"}]}]"#;
    const CLEAN: &str = r#"[{"source": "style.css", "warnings": []}]"#;
    #[test]
    fn stylelint_reports_json_warnings() {
        let findings = parse_stylelint(DIRTY.as_bytes(), Some(2), &["style.css"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "color-no-invalid-hex");
        let clean = parse_stylelint(CLEAN.as_bytes(), Some(0), &["style.css"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_stylelint(b"", Some(2), &["style.css"]).is_err());
        assert!(parse_stylelint(DIRTY.as_bytes(), Some(2), &["other.css"]).is_err());
        assert!(parse_stylelint(&[0xff], Some(0), &["x"]).is_err());
    }
}
