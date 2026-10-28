use super::{as_text, code_name, known_spelled, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "sqlfluff";

fn number(value: &serde_json::Value, key: &str) -> Option<u64> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|number| number.max(1))
}

fn finding(file: &str, violation: &serde_json::Value) -> Result<FileFinding, ParseError> {
    let rule_id = violation
        .get("code")
        .and_then(serde_json::Value::as_str)
        .filter(|code| !code.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, "violation without a code"))?;
    let message = violation
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, format!("{rule_id} without a description")))?;
    let line = number(violation, "start_line_no")
        .ok_or_else(|| ParseError::shape(TOOL, format!("{rule_id} without start_line_no")))?;
    let column = number(violation, "start_line_pos").unwrap_or(1);
    let end = number(violation, "end_line_no")
        .zip(number(violation, "end_line_pos"))
        .map(|(line, column)| TextPosition { line, column });
    let severity = match violation
        .get("warning")
        .and_then(serde_json::Value::as_bool)
    {
        Some(true) => ToolSeverity::Warning,
        _ => ToolSeverity::Error,
    };
    Ok(FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule_id.to_owned(),
            message: message.to_owned(),
            severity,
            start: TextPosition { line, column },
            end,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    })
}

/// Parses `sqlfluff lint --format json`: exit 0 is clean, exit 1 reports violations.
pub fn parse_sqlfluff(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stderr)
                    .lines()
                    .chain(String::from_utf8_lossy(stdout).lines())
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("no output")
                    .trim()
            ),
        });
    }
    let report = as_text(TOOL, stdout)?;
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(report.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut findings = Vec::new();
    for entry in &entries {
        let filepath = entry
            .get("filepath")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ParseError::shape(TOOL, "entry without a filepath"))?;
        let checked = known_spelled(TOOL, files, filepath)?;
        let violations = entry
            .get("violations")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ParseError::shape(TOOL, format!("{filepath} without violations")))?;
        for violation in violations {
            findings.push(finding(checked, violation)?);
        }
    }
    let errors = findings
        .iter()
        .any(|found| found.finding.severity == ToolSeverity::Error);
    if errors != (code == Some(1)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} does not match the {} reported finding(s)",
                code_name(code),
                findings.len()
            ),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded from sqlfluff 4.4.0 with the argv `sqlfluff_check` builds, files named `/scratch/<name>.sql`, timings removed.
    const CLEAN: &str = r#"[{"filepath": "/scratch/clean.sql", "violations": [], "statistics": {"source_chars": 16, "templated_chars": 16, "segments": 24, "raw_segments": 13}}]"#;
    const DIRTY: &str = r#"[{"filepath": "/scratch/bad.sql", "violations": [{"start_line_no": 1, "start_line_pos": 1, "code": "PRS", "description": "Line 1, Position 1: Found unparsable section: 'SELEC 1 FROM'", "name": "", "warning": false, "start_file_pos": 0, "end_line_no": 1, "end_line_pos": 13, "end_file_pos": 12}]}, {"filepath": "/scratch/dirty.sql", "violations": [{"start_line_no": 1, "start_line_pos": 1, "code": "LT09", "description": "Select targets should be on a new line unless there is only one select target.", "name": "layout.select_targets", "warning": false, "fixes": [], "start_file_pos": 0, "end_line_no": 1, "end_line_pos": 11, "end_file_pos": 10}, {"start_line_no": 1, "start_line_pos": 10, "code": "LT01", "description": "Expected single whitespace between comma ',' and naked identifier.", "name": "layout.spacing", "warning": false, "fixes": [], "start_file_pos": 9, "end_line_no": 1, "end_line_pos": 11, "end_file_pos": 10}]}, {"filepath": "/scratch/sub/q.sql", "violations": []}]"#;

    fn files() -> [&'static str; 4] {
        [
            "/scratch/clean.sql",
            "/scratch/bad.sql",
            "/scratch/dirty.sql",
            "/scratch/sub/q.sql",
        ]
    }

    #[test]
    fn clean_run_is_clean() {
        assert!(parse_sqlfluff(CLEAN.as_bytes(), b"", Some(0), &files())
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn violations_map_to_findings() {
        let findings = parse_sqlfluff(DIRTY.as_bytes(), b"", Some(1), &files()).expect("dirty");
        let summary: Vec<_> = findings
            .iter()
            .map(|found| {
                (
                    found.file.as_str(),
                    found.finding.rule_id.as_str(),
                    found.finding.start.line,
                    found.finding.start.column,
                    found.finding.end.map(|end| end.column),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("/scratch/bad.sql", "PRS", 1, 1, Some(13)),
                ("/scratch/dirty.sql", "LT09", 1, 1, Some(11)),
                ("/scratch/dirty.sql", "LT01", 1, 10, Some(11)),
            ]
        );
        assert!(findings
            .iter()
            .all(|found| found.finding.severity == ToolSeverity::Error));
    }

    #[test]
    fn warnings_alone_exit_zero() {
        let report = r#"[{"filepath": "/scratch/clean.sql", "violations": [{"start_line_no": 2, "start_line_pos": 3, "code": "LT05", "description": "Line is too long.", "warning": true}]}]"#;
        let findings = parse_sqlfluff(report.as_bytes(), b"", Some(0), &files()).expect("warn");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert!(findings[0].finding.end.is_none());
    }

    #[test]
    fn mismatched_exit_is_rejected() {
        assert!(matches!(
            parse_sqlfluff(CLEAN.as_bytes(), b"", Some(1), &files()),
            Err(ParseError::Shape { .. })
        ));
        assert!(matches!(
            parse_sqlfluff(DIRTY.as_bytes(), b"", Some(0), &files()),
            Err(ParseError::Shape { .. })
        ));
    }

    #[test]
    fn user_errors_are_incomplete() {
        let err = parse_sqlfluff(
            b"",
            b"User Error: No dialect was specified.\n",
            Some(2),
            &files(),
        )
        .expect_err("exit 2");
        assert!(
            matches!(err, ParseError::Incomplete { ref detail, .. } if detail.contains("No dialect"))
        );
    }

    #[test]
    fn unknown_files_are_rejected() {
        let report = r#"[{"filepath": "/elsewhere.sql", "violations": []}]"#;
        assert!(matches!(
            parse_sqlfluff(report.as_bytes(), b"", Some(0), &files()),
            Err(ParseError::UnknownFile { .. })
        ));
    }
}
