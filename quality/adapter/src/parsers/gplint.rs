use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity, LINE_END};

/// Parses `gplint -f json`. Every requested file must be named exactly once, because gplint
/// treats arguments as globs and silently drops what does not match, and because it exits 0
/// with no output when it crashes.
pub fn parse_gplint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "gplint";
    check_output_size(TOOL, stdout)?;
    if stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("no report (exit {})", code_name(code)),
        });
    }
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let entries = value.as_array().ok_or_else(|| ParseError::Shape {
        tool: TOOL,
        detail: "expected JSON array".to_owned(),
    })?;
    let shape = |detail: &str| ParseError::Shape {
        tool: TOOL,
        detail: detail.to_owned(),
    };
    let mut findings = Vec::new();
    let mut reported: Vec<&str> = Vec::new();
    let mut saw_error = false;
    for entry in entries {
        let path = entry
            .get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| shape("missing filePath"))?;
        let checked = known(TOOL, files, path)?;
        if reported.contains(&checked) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("{checked} was reported twice"),
            });
        }
        reported.push(checked);
        let errors = entry
            .get("errors")
            .and_then(|v| v.as_array())
            .ok_or_else(|| shape("missing errors"))?;
        for error in errors {
            let line = error
                .get("line")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| shape("missing line"))?
                .max(1); // empty files report line 0
            let column = error.get("column").and_then(|v| v.as_u64()).unwrap_or(0);
            let rule = error
                .get("rule")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| shape("missing message"))?
                .to_owned();
            let severity = match error.get("level").and_then(|v| v.as_u64()) {
                Some(2) => {
                    saw_error = true;
                    ToolSeverity::Error
                }
                Some(1) => ToolSeverity::Warning,
                other => {
                    return Err(ParseError::Shape {
                        tool: TOOL,
                        detail: format!("unknown level {other:?}"),
                    })
                }
            };
            // Parse errors have no column; they cover their whole line.
            let (start, end) = if column == 0 {
                (
                    TextPosition { line, column: 1 },
                    Some(TextPosition {
                        line,
                        column: LINE_END,
                    }),
                )
            } else {
                (TextPosition { line, column }, None)
            };
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: rule,
                    message,
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
    if let Some(missing) = files.iter().find(|file| !reported.contains(file)) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("no result for {missing}"),
        });
    }
    if code == Some(0) && saw_error {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "errors reported but exit 0".to_owned(),
        });
    }
    if !saw_error && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no errors", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = r#"[{"filePath":"/s/a.feature","errors":[{"level":2,"message":"Missing Feature name","rule":"no-unnamed-features","line":1,"column":1}]}]"#;
    const CLEAN: &str = r#"[{"filePath":"/s/a.feature","errors":[]}]"#;
    const FILES: &[&str] = &["/s/a.feature"];

    #[test]
    fn gplint_reports_json_errors() {
        let findings = parse_gplint(DIRTY.as_bytes(), Some(1), FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "no-unnamed-features");
        assert!(parse_gplint(CLEAN.as_bytes(), Some(0), FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn gplint_parse_errors_cover_their_line() {
        let report = r#"[{"filePath":"/s/a.feature","errors":[{"message":"(1:1): expected","rule":"unexpected-error","line":1,"column":0,"level":2}]}]"#;
        let findings = parse_gplint(report.as_bytes(), Some(1), FILES).expect("parsed");
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(
            findings[0].finding.end.map(|end| end.column),
            Some(LINE_END)
        );
    }

    #[test]
    fn gplint_never_reports_a_failed_run_as_clean() {
        assert!(parse_gplint(b"", Some(0), FILES).is_err());
        assert!(parse_gplint(b"", Some(1), FILES).is_err());
        assert!(parse_gplint(b"{}", Some(0), FILES).is_err());
        assert!(parse_gplint(b"not json", Some(1), FILES).is_err());
        assert!(parse_gplint(&[0xff], Some(0), FILES).is_err());
        // A file dropped by globbing or ignoring.
        assert!(parse_gplint(b"[]", Some(0), FILES).is_err());
        assert!(
            parse_gplint(CLEAN.as_bytes(), Some(0), &["/s/a.feature", "/s/b.feature"]).is_err()
        );
        // An unknown file, a duplicate, and an exit code that disagrees with the report.
        assert!(parse_gplint(DIRTY.as_bytes(), Some(1), &["/s/other.feature"]).is_err());
        let twice =
            r#"[{"filePath":"/s/a.feature","errors":[]},{"filePath":"/s/a.feature","errors":[]}]"#;
        assert!(parse_gplint(twice.as_bytes(), Some(0), FILES).is_err());
        assert!(parse_gplint(CLEAN.as_bytes(), Some(1), FILES).is_err());
        assert!(parse_gplint(DIRTY.as_bytes(), Some(0), FILES).is_err());
    }

    #[test]
    fn gplint_warnings_exit_zero() {
        let report = r#"[{"filePath":"/s/a.feature","errors":[{"level":1,"message":"m","rule":"r","line":2,"column":3}]}]"#;
        let findings = parse_gplint(report.as_bytes(), Some(0), FILES).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
    }

    /// gplint counts columns in Unicode scalars: `@a` after an astral tag is column 9.
    #[test]
    fn gplint_columns_are_scalars_after_astral_characters() {
        let source = "  @\u{1f600} @a @a\n";
        let report = r#"[{"filePath":"/s/a.feature","errors":[{"level":2,"message":"m","rule":"no-duplicate-tags","line":1,"column":9}]}]"#;
        let findings = parse_gplint(report.as_bytes(), Some(1), FILES).expect("parsed");
        let placed =
            crate::place_finding(&findings[0].finding, "a.feature", source).expect("placed");
        assert_eq!(placed.start_byte, Some(11));
    }
}
