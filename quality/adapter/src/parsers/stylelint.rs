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
    let mut reported: Vec<&str> = Vec::new();
    for entry in entries {
        let source = entry
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: "missing source".to_owned(),
            })?;
        let checked = known(TOOL, files, source)?;
        reported.push(checked);
        if entry.get("ignored").and_then(|v| v.as_bool()) == Some(true) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("{checked} was ignored, so it was not checked"),
            });
        }
        if let Some(option) = entry
            .get("invalidOptionWarnings")
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
        {
            let text = option.get("text").and_then(|v| v.as_str()).unwrap_or("");
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("invalid configuration: {text}"),
            });
        }
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
                    encoding: ColumnEncoding::Utf16,
                    related: Vec::new(),
                },
            });
        }
    }
    if let Some(missing) = files.iter().find(|file| !reported.contains(file)) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("no result for {missing}"),
        });
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

    #[test]
    fn stylelint_never_reports_an_unchecked_file_as_clean() {
        let ignored = r#"[{"source": "style.css", "ignored": true, "warnings": []}]"#;
        assert!(parse_stylelint(ignored.as_bytes(), Some(0), &["style.css"]).is_err());
        let invalid = r#"[{"source": "style.css", "invalidOptionWarnings": [{"text": "Invalid option"}], "warnings": []}]"#;
        let error = parse_stylelint(invalid.as_bytes(), Some(2), &["style.css"])
            .expect_err("invalid options");
        assert!(error.to_string().contains("Invalid option"), "{error}");
        assert!(parse_stylelint(CLEAN.as_bytes(), Some(0), &["style.css", "other.css"]).is_err());
    }

    #[test]
    fn stylelint_columns_are_utf16_after_astral_characters() {
        let source = "a { content: \"\u{1f600}\"; color: #ffg; }\n";
        let report = r#"[{"source":"/s/a.css","warnings":[{"line":1,"column":27,"rule":"color-no-invalid-hex","text":"m","severity":"error"}]}]"#;
        let findings = parse_stylelint(report.as_bytes(), Some(2), &["/s/a.css"]).expect("parsed");
        let placed = crate::place_finding(&findings[0].finding, "a.css", source).expect("placed");
        assert_eq!(placed.start_byte, Some(28));
        let end = "x\u{1f600}\n";
        let report = r#"[{"source":"/s/a.css","warnings":[{"line":1,"column":4,"rule":"r","text":"m","severity":"error"}]}]"#;
        let findings = parse_stylelint(report.as_bytes(), Some(2), &["/s/a.css"]).expect("parsed");
        let placed = crate::place_finding(&findings[0].finding, "a.css", end).expect("placed");
        assert_eq!(placed.start_byte, Some(5));
    }
}
