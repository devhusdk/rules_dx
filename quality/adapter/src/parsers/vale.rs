use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct ValeAlert {
    #[serde(rename = "Span")]
    span: Vec<u64>,
    #[serde(rename = "Check")]
    check: String,
    #[serde(rename = "Message")]
    message: String,
    #[serde(rename = "Severity")]
    severity: String,
    #[serde(rename = "Line")]
    line: u64,
}

fn vale_severity(level: &str) -> Result<ToolSeverity, ParseError> {
    match level {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        "suggestion" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::Shape {
            tool: "vale",
            detail: format!("unknown severity: {level}"),
        }),
    }
}

pub fn parse_vale(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "vale";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if text.trim().is_empty() {
        return Err(ParseError::Json {
            tool: TOOL,
            detail: "empty output".to_owned(),
        });
    }
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let report = value.as_object().ok_or_else(|| ParseError::Json {
        tool: TOOL,
        detail: "top-level JSON is not an object".to_owned(),
    })?;
    if let Some(code_value) = report.get("Code") {
        let detail = report
            .get("Text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("vale reported a configuration error");
        let lowered = detail.to_ascii_lowercase();
        if ["rst2html", "asciidoctor"]
            .iter()
            .any(|converter| lowered.contains(converter))
        {
            return Err(ParseError::Incomplete {
                tool: TOOL,
                detail: format!("vale document converter failed: {}", detail.trim()),
            });
        }
        return Err(ParseError::ValeConfig {
            detail: format!("{}: {}", code_value, detail.trim()),
        });
    }
    let mut findings = Vec::new();
    let mut paths: Vec<&String> = report.keys().collect();
    paths.sort();
    for path in paths {
        let checked = known(TOOL, files, path)?;
        let alerts: Vec<ValeAlert> =
            serde_json::from_value(report[path].clone()).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
        for alert in &alerts {
            let (start_column, end_column) = match alert.span.as_slice() {
                [start, end] if *start >= 1 && *end >= *start => (*start, *end + 1),
                _ => {
                    return Err(ParseError::Shape {
                        tool: TOOL,
                        detail: format!("malformed span for {}: {:?}", alert.check, alert.span),
                    });
                }
            };
            if alert.line == 0 {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("zero line for {}", alert.check),
                });
            }
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: alert.check.clone(),
                    message: alert.message.clone(),
                    severity: vale_severity(&alert.severity)?,
                    start: TextPosition {
                        line: alert.line,
                        column: start_column,
                    },
                    end: Some(TextPosition {
                        line: alert.line,
                        column: end_column,
                    }),
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
            detail: format!("exit {} with no parsed alerts", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALE_DIRTY: &str = r#"{
  "/s/vale/doc.md": [
    {
      "Action": {"Name": "", "Params": null},
      "Span": [9, 14],
      "Check": "Test.Cotton",
      "Description": "",
      "Link": "https://example.com",
      "Message": "Avoid cotton.",
      "Severity": "error",
      "Match": "cotton",
      "Line": 1
    }
  ]
}"#;

    const VALE_CONFIG_ERROR: &str = r#"{
  "Line": 0,
  "Path": "",
  "Text": "E100 [--config] Runtime error\n\npath '/s/nope.ini' does not exist\n\nExecution stopped with code 1.",
  "Code": "E100",
  "Span": 0
}"#;

    #[test]
    fn vale_reports_alerts_with_inclusive_spans() {
        let findings =
            parse_vale(VALE_DIRTY.as_bytes(), Some(1), &["/s/vale/doc.md"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "Test.Cotton");
        assert_eq!(findings[0].finding.message, "Avoid cotton.");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 1, column: 9 },
                Some(TextPosition {
                    line: 1,
                    column: 15
                })
            )
        );
        assert!(parse_vale(b"{}", Some(0), &["/s/vale/clean.md"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn vale_surfaces_config_envelopes() {
        let err = parse_vale(VALE_CONFIG_ERROR.as_bytes(), Some(2), &["/s/vale/doc.md"])
            .expect_err("config error");
        assert!(matches!(err, ParseError::ValeConfig { .. }));
        assert!(err.to_string().contains("nope.ini"));
        assert!(parse_vale(b"", Some(2), &["/s/vale/doc.md"]).is_err());
        assert!(parse_vale(VALE_DIRTY.as_bytes(), Some(1), &["/s/other.md"]).is_err());
    }

    #[test]
    fn vale_converter_failures_are_incomplete_not_config() {
        for text in ["rst2html not found", "asciidoctor not found"] {
            let envelope = format!(r#"{{"Code": "E100", "Text": "{text}"}}"#);
            let err = parse_vale(envelope.as_bytes(), Some(2), &["/s/vale/doc.rst"])
                .expect_err("converter failure");
            assert!(matches!(err, ParseError::Incomplete { .. }), "{err}");
            assert!(err.to_string().contains(text.split(' ').next().unwrap()));
        }
    }

    #[test]
    fn vale_rejects_bytes_json_shapes_and_alert_fields() {
        assert!(parse_vale(b"\xff", Some(2), &["/s/x.md"]).is_err());
        assert!(parse_vale(b"{nope", Some(2), &["/s/x.md"]).is_err());
        assert!(parse_vale(b"[1,2]", Some(2), &["/s/x.md"]).is_err());
        assert!(parse_vale(b"{}", Some(1), &["/s/x.md"]).is_err());
        let missing_line = r#"{"/s/x.md": [{"Span": [9, 14], "Check": "T.C", "Message": "m", "Severity": "error"}]}"#;
        assert!(parse_vale(missing_line.as_bytes(), Some(1), &["/s/x.md"]).is_err());
        for span in ["[0, 2]", "[3, 2]", "[1]", "[]"] {
            let alert = format!(
                r#"{{"/s/x.md": [{{"Span": {span}, "Check": "T.C", "Message": "m", "Severity": "error", "Line": 1}}]}}"#
            );
            assert!(
                parse_vale(alert.as_bytes(), Some(1), &["/s/x.md"]).is_err(),
                "span: {span}"
            );
        }
        let zero_line = r#"{
            "/s/x.md": [{"Span": [9, 14], "Check": "T.C", "Message": "m", "Severity": "error", "Line": 0}]
        }"#;
        assert!(parse_vale(zero_line.as_bytes(), Some(1), &["/s/x.md"]).is_err());
        let fatal = r#"{
            "/s/x.md": [{"Span": [9, 14], "Check": "T.C", "Message": "m", "Severity": "fatal", "Line": 1}]
        }"#;
        assert!(parse_vale(fatal.as_bytes(), Some(1), &["/s/x.md"]).is_err());
        let levels = r#"{
            "/s/x.md": [
                {"Span": [1, 2], "Check": "T.W", "Message": "w", "Severity": "warning", "Line": 1},
                {"Span": [1, 2], "Check": "T.S", "Message": "s", "Severity": "suggestion", "Line": 2}
            ]
        }"#;
        let findings = parse_vale(levels.as_bytes(), Some(1), &["/s/x.md"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Info);
    }
}
