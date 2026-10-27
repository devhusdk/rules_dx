use super::{as_text, code_name, known_spelled, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "statix";

/// statix reports a parse failure as a code 0 report.
const SYNTAX_CODE: u64 = 0;

fn field<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a serde_json::Value, ParseError> {
    value
        .get(key)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| ParseError::shape(TOOL, format!("{key} is not a string")))
}

fn position(span: &serde_json::Value, key: &str) -> Result<TextPosition, ParseError> {
    let at = field(span, key)?;
    let number = |name: &str| {
        field(at, name)?
            .as_u64()
            .map(|number| number.max(1))
            .ok_or_else(|| ParseError::shape(TOOL, format!("{key}.{name} is not a number")))
    };
    Ok(TextPosition {
        line: number("line")?,
        column: number("column")?,
    })
}

/// Parses `statix check -o json <file>`.
///
/// Exit 1 prints one JSON report per file with findings; exit 0 with empty output is clean.
/// statix prints config and IO errors (stderr) and unreadable files (stdout text) and still
/// exits 0, so any stderr or non-JSON stdout fails closed.
pub fn parse_statix(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {stderr}", code_name(code)),
        ));
    }
    if !stderr.is_empty() {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("statix failed: {stderr}"),
        });
    }
    let report = as_text(TOOL, stdout)?.trim();
    let mut findings = Vec::new();
    if !report.is_empty() {
        if !report.starts_with('{') {
            return Err(ParseError::Incomplete {
                tool: TOOL,
                detail: format!(
                    "statix could not check the file: {}",
                    report.lines().next().unwrap_or_default()
                ),
            });
        }
        for value in serde_json::Deserializer::from_str(report).into_iter::<serde_json::Value>() {
            let value = value.map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
            file_findings(&value, files, &mut findings)?;
        }
    }
    if code == Some(0) && !findings.is_empty() {
        return Err(ParseError::shape(TOOL, "exit 0 with diagnostics"));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

fn file_findings(
    value: &serde_json::Value,
    files: &[&str],
    findings: &mut Vec<FileFinding>,
) -> Result<(), ParseError> {
    let checked = known_spelled(TOOL, files, text(value, "file")?)?;
    let reports = field(value, "report")?
        .as_array()
        .ok_or_else(|| ParseError::shape(TOOL, "report is not an array"))?;
    for report in reports {
        let code = field(report, "code")?
            .as_u64()
            .ok_or_else(|| ParseError::shape(TOOL, "code is not a number"))?;
        let note = text(report, "note")?.trim();
        let (rule_id, severity) = if code == SYNTAX_CODE {
            ("syntax".to_owned(), ToolSeverity::Error)
        } else {
            let severity = match text(report, "severity")? {
                "Error" => ToolSeverity::Error,
                "Warn" => ToolSeverity::Warning,
                "Hint" => ToolSeverity::Info,
                other => {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("unknown severity {other:?}"),
                    ))
                }
            };
            (format!("W{code:02}"), severity)
        };
        let diagnostics = field(report, "diagnostics")?
            .as_array()
            .ok_or_else(|| ParseError::shape(TOOL, "diagnostics is not an array"))?;
        if diagnostics.is_empty() {
            return Err(ParseError::shape(TOOL, "report without diagnostics"));
        }
        for diagnostic in diagnostics {
            let at = field(diagnostic, "at")?;
            let start = position(at, "from")?;
            let end = position(at, "to")?;
            let detail = text(diagnostic, "message")?.trim();
            let message = if code == SYNTAX_CODE || note.is_empty() || detail.is_empty() {
                if detail.is_empty() { note } else { detail }.to_owned()
            } else {
                format!("{note}: {detail}")
            };
            if message.is_empty() {
                return Err(ParseError::shape(TOOL, "missing message"));
            }
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: rule_id.clone(),
                    message,
                    severity,
                    end: (end != start).then_some(end),
                    start,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf8,
                    related: Vec::new(),
                },
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{
  "file": "dirty.nix",
  "report": [
    {
      "note": "Useless let-in expression",
      "code": 2,
      "severity": "Warn",
      "diagnostics": [
        {
          "at": {"from": {"line": 4, "column": 22}, "to": {"line": 4, "column": 30}},
          "message": "This let-in expression has no entries",
          "suggestion": {"at": {"from": {"line": 4, "column": 22}, "to": {"line": 4, "column": 30}}, "fix": "1"}
        }
      ]
    }
  ]
}
"#;
    const SYNTAX: &str = r#"{"file":"syntax.nix","report":[{"note":"Syntax error","code":0,"severity":"Error","diagnostics":[{"at":{"from":{"line":1,"column":7},"to":{"line":1,"column":8}},"message":"Unexpected TOKEN_SEMICOLON at 6..7","suggestion":null}]}]}"#;

    #[test]
    fn statix_clean_run_is_clean() {
        assert!(parse_statix(b"", b"", Some(0), &["a.nix"])
            .expect("parsed")
            .is_empty());
        assert!(parse_statix(b"\n", b"", Some(0), &["a.nix"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn statix_reports_lint_findings() {
        let findings =
            parse_statix(DIRTY.as_bytes(), b"", Some(1), &["dirty.nix"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0].finding;
        assert_eq!(findings[0].file, "dirty.nix");
        assert_eq!(finding.rule_id, "W02");
        assert_eq!(finding.severity, ToolSeverity::Warning);
        assert_eq!(
            finding.message,
            "Useless let-in expression: This let-in expression has no entries"
        );
        assert_eq!(
            finding.start,
            TextPosition {
                line: 4,
                column: 22
            }
        );
        assert_eq!(
            finding.end,
            Some(TextPosition {
                line: 4,
                column: 30
            })
        );
        assert_eq!(finding.encoding, ColumnEncoding::Utf8);
    }

    #[test]
    fn statix_syntax_errors_are_error_findings() {
        let findings =
            parse_statix(SYNTAX.as_bytes(), b"", Some(1), &["syntax.nix"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            findings[0].finding.message,
            "Unexpected TOKEN_SEMICOLON at 6..7"
        );
    }

    #[test]
    fn statix_errors_that_exit_zero_are_never_clean() {
        let config =
            b"config error: unable to parse config file: TOML parse error at line 1, column 13\n";
        let err = parse_statix(b"", config, Some(0), &["a.nix"]).expect_err("config error");
        assert!(err.to_string().contains("config error"), "{err}");
        let missing = b"config error: path error: No such file or directory (os error 2)\n";
        assert!(parse_statix(b"", missing, Some(0), &["a.nix"]).is_err());
        let unreadable = b"`a.nix` contains non-utf8 content\n";
        let err = parse_statix(unreadable, b"", Some(0), &["a.nix"]).expect_err("unreadable");
        assert!(err.to_string().contains("non-utf8"), "{err}");
    }

    #[test]
    fn statix_rejects_malformed_output_and_exit_codes() {
        assert!(parse_statix(b"{\"file\":", b"", Some(1), &["a.nix"]).is_err());
        assert!(parse_statix(b"", b"", Some(1), &["a.nix"]).is_err());
        assert!(parse_statix(b"", b"error: unexpected argument\n", Some(2), &["a.nix"]).is_err());
        assert!(parse_statix(b"", b"", None, &["a.nix"]).is_err());
        assert!(parse_statix(DIRTY.as_bytes(), b"", Some(0), &["dirty.nix"]).is_err());
        assert!(parse_statix(&[0xff], b"", Some(1), &["a.nix"]).is_err());
        let unknown = DIRTY.replace("\"Warn\"", "\"Fatal\"");
        assert!(parse_statix(unknown.as_bytes(), b"", Some(1), &["dirty.nix"]).is_err());
        let empty = r#"{"file":"dirty.nix","report":[{"note":"x","code":2,"severity":"Warn","diagnostics":[]}]}"#;
        assert!(parse_statix(empty.as_bytes(), b"", Some(1), &["dirty.nix"]).is_err());
    }

    #[test]
    fn statix_rejects_unknown_files() {
        let err =
            parse_statix(DIRTY.as_bytes(), b"", Some(1), &["other.nix"]).expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
