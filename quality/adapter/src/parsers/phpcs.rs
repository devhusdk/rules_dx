use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, code_name, finding, FileFinding, ParseError};

const TOOL: &str = "phpcs";

/// Findings from `phpcs --report=json`.
///
/// The exit code is a bitmask: 0 is clean, 1 is fixable findings, 2 is non-fixable findings and
/// 3 is both. Anything else (16 for a process error, 64 for unmet requirements, a signal) fails
/// the run, and so does output that is not a report. `files` is keyed by the path phpcs checked,
/// so it may only name the owned `files`; a clean run lists none. The totals must agree with the
/// counted messages, and a message from phpcs itself (`Internal.Exception`,
/// `Internal.Tokenizer.Exception`) means the file was not checked, so it fails the run too. The
/// rule is `phpcs/<source>`, the severity follows the message `type`.
pub fn parse_phpcs(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    if !matches!(code, Some(0..=3)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), excerpt(text)),
        ));
    }
    let report: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|err| ParseError::shape(TOOL, format!("output is not JSON: {err}")))?;
    let totals = report
        .get("totals")
        .ok_or_else(|| ParseError::shape(TOOL, "report without totals".to_owned()))?;
    let total = |key: &str| {
        totals
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ParseError::shape(TOOL, format!("totals without {key}")))
    };
    let (total_errors, total_warnings) = (total("errors")?, total("warnings")?);
    // PHP encodes an empty `files` as an array rather than an object.
    let empty = serde_json::Map::new();
    let by_file = match report.get("files") {
        Some(serde_json::Value::Object(by_file)) => by_file,
        Some(serde_json::Value::Array(none)) if none.is_empty() => &empty,
        _ => return Err(ParseError::shape(TOOL, "report without files".to_owned())),
    };
    let mut findings: Vec<FileFinding> = Vec::new();
    let (mut errors, mut warnings) = (0u64, 0u64);
    for (path, entry) in by_file {
        if !files.contains(&path.as_str()) {
            return Err(ParseError::shape(
                TOOL,
                format!("report names {path}, which was not checked"),
            ));
        }
        let messages = entry
            .get("messages")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ParseError::shape(TOOL, format!("{path} without messages")))?;
        for message in messages {
            let (found, is_error) = one(path, message)?;
            if is_error {
                errors += 1;
            } else {
                warnings += 1;
            }
            findings.push(found);
        }
    }
    if (errors, warnings) != (total_errors, total_warnings) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "totals {total_errors} error(s) and {total_warnings} warning(s) disagree with {errors} and {warnings} counted"
            ),
        ));
    }
    match (code, findings.is_empty()) {
        // A bound ruleset may mask issues from the exit code
        // (`ignore_warnings_on_exit` and friends), so exit 0 can come with
        // findings; the totals check above is the consistency guard.
        (Some(1..=3), true) => Err(ParseError::shape(
            TOOL,
            format!("exit {} with no diagnostics", code_name(code)),
        )),
        _ => {
            findings.sort_by(|a, b| {
                (
                    &a.file,
                    a.finding.start.line,
                    a.finding.start.column,
                    &a.finding.rule_id,
                )
                    .cmp(&(
                        &b.file,
                        b.finding.start.line,
                        b.finding.start.column,
                        &b.finding.rule_id,
                    ))
            });
            Ok(findings)
        }
    }
}

fn one(path: &str, message: &serde_json::Value) -> Result<(FileFinding, bool), ParseError> {
    let text = message
        .get("message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} message is not text")))?;
    let source = message
        .get("source")
        .and_then(serde_json::Value::as_str)
        .filter(|source| !source.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} message without source")))?;
    if source == "Internal.Exception" || source.starts_with("Internal.Tokenizer.Exception") {
        return Err(ParseError::shape(
            TOOL,
            format!("{path} was not checked: {text}"),
        ));
    }
    let (severity, is_error) = match message.get("type").and_then(serde_json::Value::as_str) {
        Some("ERROR") => (ToolSeverity::Error, true),
        Some("WARNING") => (ToolSeverity::Warning, false),
        _ => {
            return Err(ParseError::shape(
                TOOL,
                format!("{path} message without a known type"),
            ))
        }
    };
    let position = |key: &str| {
        message
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value.max(1))
            .ok_or_else(|| ParseError::shape(TOOL, format!("{path} message without {key}")))
    };
    Ok((
        finding(
            TOOL,
            path,
            format!("phpcs/{source}"),
            text.to_owned(),
            severity,
            TextPosition {
                line: position("line")?,
                column: position("column")?,
            },
            ColumnEncoding::Scalars,
        ),
        is_error,
    ))
}

/// The first lines of output, enough to name why a run failed.
fn excerpt(out: &str) -> String {
    let text = out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("; ");
    if text.is_empty() {
        "no output".to_owned()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"totals":{"errors":1,"warnings":1,"fixable":1},"files":{"/s/a.php":{"errors":1,"warnings":1,"messages":[{"message":"Line exceeds 120 characters","source":"Generic.Files.LineLength.TooLong","severity":5,"fixable":false,"type":"WARNING","line":9,"column":121},{"message":"Missing blank line after opening tag","source":"PSR12.Files.OpenTag.NotAlone","severity":5,"fixable":true,"type":"ERROR","line":2,"column":1}]}}}"#;

    fn internal(source: &str) -> String {
        format!(
            r#"{{"totals":{{"errors":1,"warnings":0,"fixable":0}},"files":{{"/s/a.php":{{"errors":1,"warnings":0,"messages":[{{"message":"boom","source":"{source}","severity":5,"fixable":false,"type":"ERROR","line":1,"column":1}}]}}}}}}"#
        )
    }

    #[test]
    fn phpcs_reports_errors_and_warnings_with_positions_and_rules() {
        for code in [1, 2, 3] {
            let found = parse_phpcs(REPORT.as_bytes(), Some(code), &["/s/a.php"]).expect("parsed");
            let seen: Vec<_> = found
                .iter()
                .map(|f| {
                    (
                        f.finding.start.line,
                        f.finding.start.column,
                        f.finding.rule_id.as_str(),
                        f.finding.severity,
                    )
                })
                .collect();
            assert_eq!(
                seen,
                vec![
                    (
                        2,
                        1,
                        "phpcs/PSR12.Files.OpenTag.NotAlone",
                        ToolSeverity::Error
                    ),
                    (
                        9,
                        121,
                        "phpcs/Generic.Files.LineLength.TooLong",
                        ToolSeverity::Warning
                    ),
                ]
            );
        }
    }

    #[test]
    fn phpcs_clean_run_has_no_findings() {
        let out = br#"{"totals":{"errors":0,"warnings":0,"fixable":0},"files":[]}"#;
        assert!(parse_phpcs(out, Some(0), &["/s/a.php"])
            .expect("clean")
            .is_empty());
        let empty = br#"{"totals":{"errors":0,"warnings":0,"fixable":0},"files":{}}"#;
        assert!(parse_phpcs(empty, Some(0), &[]).expect("clean").is_empty());
    }

    #[test]
    fn phpcs_fails_closed_on_other_exit_codes() {
        for code in [Some(4), Some(16), Some(64), Some(255), None] {
            let err = parse_phpcs(b"ERROR: boom\n", code, &["/s/a.php"]).expect_err("failed");
            assert!(err.to_string().contains("ERROR: boom"), "{err}");
        }
        parse_phpcs(REPORT.as_bytes(), Some(16), &["/s/a.php"]).expect_err("bitmask above 3");
    }

    #[test]
    fn phpcs_accepts_warnings_at_exit_zero_when_a_ruleset_masks_them() {
        let warnings = r#"{"totals":{"errors":0,"warnings":1,"fixable":0},"files":{"/s/a.php":{"errors":0,"warnings":1,"messages":[{"message":"Line exceeds 120 characters","source":"Generic.Files.LineLength.TooLong","severity":5,"fixable":false,"type":"WARNING","line":9,"column":121}]}}}"#;
        let found = parse_phpcs(warnings.as_bytes(), Some(0), &["/s/a.php"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        let skewed = warnings.replace(r#""warnings":1,"fixable""#, r#""warnings":0,"fixable""#);
        parse_phpcs(skewed.as_bytes(), Some(0), &["/s/a.php"]).expect_err("totals disagree");
    }

    #[test]
    fn phpcs_fails_closed_on_inconsistent_reports() {
        let files = ["/s/a.php"];
        parse_phpcs(b"not json", Some(1), &files).expect_err("not JSON");
        parse_phpcs(b"[]", Some(1), &files).expect_err("not a report");
        let clean = br#"{"totals":{"errors":0,"warnings":0,"fixable":0},"files":[]}"#;
        parse_phpcs(clean, Some(2), &files).expect_err("non-zero without findings");
        parse_phpcs(REPORT.as_bytes(), Some(1), &["/s/other.php"]).expect_err("unchecked file");
        let skewed = REPORT.replace(
            r#""errors":1,"warnings":1,"fixable""#,
            r#""errors":2,"warnings":1,"fixable""#,
        );
        parse_phpcs(skewed.as_bytes(), Some(1), &files).expect_err("totals disagree");
    }

    #[test]
    fn phpcs_internal_exceptions_are_failures_but_no_code_found_is_a_finding() {
        let files = ["/s/a.php"];
        for source in ["Internal.Exception", "Internal.Tokenizer.Exception"] {
            let err =
                parse_phpcs(internal(source).as_bytes(), Some(2), &files).expect_err("unchecked");
            assert!(err.to_string().contains("was not checked"), "{err}");
        }
        let found = parse_phpcs(internal("Internal.NoCodeFound").as_bytes(), Some(2), &files)
            .expect("finding");
        assert_eq!(found[0].finding.rule_id, "phpcs/Internal.NoCodeFound");
    }
}
