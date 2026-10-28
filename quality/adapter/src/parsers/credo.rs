use super::{as_text, code_name, known_spelled, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "credo";
const SUMMARY_PREFIX: &str = "dx-credo: ";
const CHECK_PREFIX: &str = "Elixir.";

fn field<'a>(
    value: &'a serde_json::Value,
    key: &str,
    what: &str,
) -> Result<&'a serde_json::Value, ParseError> {
    value
        .get(key)
        .ok_or_else(|| ParseError::shape(TOOL, format!("{what} without {key}")))
}

fn text<'a>(value: &'a serde_json::Value, key: &str, what: &str) -> Result<&'a str, ParseError> {
    field(value, key, what)?
        .as_str()
        .ok_or_else(|| ParseError::shape(TOOL, format!("{what} {key} is not text")))
}

/// A 1-based position; a missing or null number (Credo omits columns for some checks) is 1.
fn number(value: &serde_json::Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn finding(
    file: &str,
    rule_id: String,
    message: &str,
    severity: ToolSeverity,
    start: TextPosition,
) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id,
            message: message.to_owned(),
            severity,
            start,
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

/// The one `dx-credo: {...}` line the driver writes to stderr, as its `invalid` list.
fn invalid_files(stderr: &[u8]) -> Result<Vec<serde_json::Value>, ParseError> {
    let stderr = as_text(TOOL, stderr)?;
    let mut summaries = stderr
        .lines()
        .filter_map(|line| line.strip_prefix(SUMMARY_PREFIX))
        .filter(|rest| rest.starts_with('{'));
    let (Some(summary), None) = (summaries.next(), summaries.next()) else {
        return Err(ParseError::shape(
            TOOL,
            "stderr must carry exactly one dx-credo summary line",
        ));
    };
    let summary: serde_json::Value =
        serde_json::from_str(summary).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("dx-credo summary ({err})"),
        })?;
    summary
        .get("invalid")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| ParseError::shape(TOOL, "dx-credo summary without invalid list"))
}

/// Parses `credo --format json` run through the dx driver.
///
/// `files` are the checked paths in the spelling the run was given. The driver exits 0 for
/// no issues and no unparsable files, 1 for at least one of either, and 2 when the run
/// halted or crashed. Anything else, a missing or malformed summary line, or an exit code
/// that disagrees with the report is an error, so a file Credo dropped is never read as clean.
pub fn parse_credo(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        let tail: String = String::from_utf8_lossy(stderr)
            .trim()
            .chars()
            .rev()
            .take(300)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {tail}", code_name(code)),
        ));
    }
    let report: serde_json::Value =
        serde_json::from_str(as_text(TOOL, stdout)?.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON object ({err})"),
        })?;
    let issues = report
        .get("issues")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing issues"))?;
    let invalid = invalid_files(stderr)?;
    let mut findings = Vec::with_capacity(issues.len() + invalid.len());
    for issue in issues {
        let check = text(issue, "check", "issue")?;
        let name = check.strip_prefix(CHECK_PREFIX).unwrap_or(check);
        let name = name.strip_prefix("Credo.Check.").unwrap_or(name);
        let file = known_spelled(TOOL, files, text(issue, "filename", "issue")?)?;
        findings.push(finding(
            file,
            format!("credo/{name}"),
            text(issue, "message", "issue")?,
            ToolSeverity::Warning,
            TextPosition {
                line: number(issue, "line_no"),
                column: number(issue, "column"),
            },
        ));
    }
    for entry in &invalid {
        let file = known_spelled(TOOL, files, text(entry, "path", "invalid file")?)?;
        findings.push(finding(
            file,
            "credo/syntax".to_owned(),
            text(entry, "message", "invalid file")?,
            ToolSeverity::Error,
            TextPosition {
                line: number(entry, "line"),
                column: number(entry, "column"),
            },
        ));
    }
    match (code, findings.is_empty()) {
        (Some(0), false) => Err(ParseError::shape(
            TOOL,
            "exit 0 with findings in the report",
        )),
        (Some(1), true) => Err(ParseError::shape(
            TOOL,
            "exit 1 without a finding in the report",
        )),
        _ => Ok(findings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[&str] = &["lib/a.ex", "lib/b.ex"];
    const NONE: &[u8] = b"dx-credo: {\"invalid\":[]}\n";
    const ISSUES: &[u8] = br#"{"issues":[{"category":"readability","check":"Credo.Check.Readability.ModuleDoc","column":11,"column_end":14,"filename":"lib/a.ex","line_no":1,"message":"Modules should have a @moduledoc tag.","priority":1,"scope":"Foo","trigger":"Foo"}]}"#;

    #[test]
    fn clean_run_is_clean() {
        let found = parse_credo(br#"{"issues":[]}"#, NONE, Some(0), FILES).expect("clean");
        assert!(found.is_empty());
    }

    #[test]
    fn issues_become_non_fixable_warnings() {
        let found = parse_credo(ISSUES, NONE, Some(1), FILES).expect("issues");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "lib/a.ex");
        assert_eq!(found[0].finding.rule_id, "credo/Readability.ModuleDoc");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.column, 11);
        assert!(found[0].finding.suggestions.is_empty());
    }

    #[test]
    fn null_columns_default_to_one() {
        let out = String::from_utf8_lossy(ISSUES).replace("\"column\":11", "\"column\":null");
        let found = parse_credo(out.as_bytes(), NONE, Some(1), FILES).expect("issues");
        assert_eq!(found[0].finding.start.column, 1);
    }

    #[test]
    fn unparsable_files_are_syntax_errors() {
        let err = b"info: x\ndx-credo: {\"invalid\":[{\"path\":\"lib/b.ex\",\"line\":2,\"column\":8,\"message\":\"missing terminator: )\"}]}\n";
        let found = parse_credo(br#"{"issues":[]}"#, err, Some(1), FILES).expect("invalid");
        assert_eq!(found[0].file, "lib/b.ex");
        assert_eq!(found[0].finding.rule_id, "credo/syntax");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 2);
    }

    #[test]
    fn fails_closed_on_odd_output() {
        let empty = br#"{"issues":[]}"#;
        assert!(parse_credo(empty, NONE, Some(1), FILES).is_err());
        assert!(parse_credo(ISSUES, NONE, Some(0), FILES).is_err());
        assert!(parse_credo(empty, NONE, Some(2), FILES).is_err());
        assert!(parse_credo(empty, NONE, Some(128), FILES).is_err());
        assert!(parse_credo(empty, NONE, None, FILES).is_err());
        assert!(parse_credo(empty, b"", Some(0), FILES).is_err());
        assert!(parse_credo(empty, b"dx-credo: {not json}\n", Some(0), FILES).is_err());
        assert!(parse_credo(empty, b"dx-credo: {}\n", Some(0), FILES).is_err());
        let twice = [NONE, NONE].concat();
        assert!(parse_credo(empty, &twice, Some(0), FILES).is_err());
        assert!(parse_credo(b"", NONE, Some(0), FILES).is_err());
        assert!(parse_credo(b"[]", NONE, Some(0), FILES).is_err());
        assert!(parse_credo(&[0xff], NONE, Some(0), FILES).is_err());
    }

    #[test]
    fn rejects_files_that_were_not_requested() {
        let err = parse_credo(ISSUES, NONE, Some(1), &["other.ex"]).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
