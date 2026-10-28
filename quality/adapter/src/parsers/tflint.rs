use super::{as_text, code_name, known_spelled, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "tflint";

fn severity(value: &serde_json::Value) -> Result<ToolSeverity, ParseError> {
    match value.as_str() {
        Some("error") => Ok(ToolSeverity::Error),
        Some("warning") => Ok(ToolSeverity::Warning),
        Some("info" | "notice") => Ok(ToolSeverity::Info),
        other => Err(ParseError::shape(
            TOOL,
            format!("unknown severity {other:?}"),
        )),
    }
}

fn number(value: &serde_json::Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn position(value: &serde_json::Value) -> TextPosition {
    TextPosition {
        line: number(value, "line"),
        column: number(value, "column"),
    }
}

/// The checked file and the span of a tflint `range` object.
fn located<'a>(
    files: &[&'a str],
    range: &serde_json::Value,
) -> Result<(&'a str, TextPosition, Option<TextPosition>), ParseError> {
    let name = range
        .get("filename")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, "range without filename"))?;
    let file = known_spelled(TOOL, files, name)?;
    let start = position(range.get("start").unwrap_or(&serde_json::Value::Null));
    let end = range
        .get("end")
        .map(position)
        .filter(|end| (end.line, end.column) > (start.line, start.column));
    Ok((file, start, end))
}

fn finding(
    file: &str,
    rule_id: &str,
    message: String,
    severity: ToolSeverity,
    span: (TextPosition, Option<TextPosition>),
) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule_id.to_owned(),
            message,
            severity,
            start: span.0,
            end: span.1,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

/// Parses `tflint --format=json` for one module directory.
///
/// `files` are the checked paths in the spelling tflint reports (it runs with `--chdir`
/// from the scratch root, so they are root-relative). Exit codes follow tflint v0.65.0:
/// 0 clean, 1 failure with the reason in JSON `errors`, 2 issues found. An `errors` entry
/// with a range (HCL diagnostics) is an error finding on that file; one without a range
/// (config, plugin or internal failure) fails the run, so it is never read as clean.
pub fn parse_tflint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0..=2)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), preview(stdout)),
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
    let errors = report
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing errors"))?;
    let mut findings = Vec::with_capacity(issues.len() + errors.len());
    for issue in issues {
        let rule = issue
            .get("rule")
            .ok_or_else(|| ParseError::shape(TOOL, "issue without rule"))?;
        let name = rule
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| ParseError::shape(TOOL, "issue without rule name"))?;
        let message = issue
            .get("message")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ParseError::shape(TOOL, "issue without message"))?;
        let range = issue
            .get("range")
            .ok_or_else(|| ParseError::shape(TOOL, "issue without range"))?;
        let (file, start, end) = located(files, range)?;
        findings.push(finding(
            file,
            name,
            message.to_owned(),
            severity(rule.get("severity").unwrap_or(&serde_json::Value::Null))?,
            (start, end),
        ));
    }
    for error in errors {
        let summary = error
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let text = match (summary.is_empty(), message.is_empty()) {
            (false, false) => format!("{summary}: {message}"),
            (false, true) => summary.to_owned(),
            _ => message.to_owned(),
        };
        let Some(range) = error.get("range").filter(|range| !range.is_null()) else {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "tflint failed: {}",
                    text.chars().take(300).collect::<String>()
                ),
            ));
        };
        let (file, start, end) = located(files, range)?;
        findings.push(finding(
            file,
            "tflint-diagnostic",
            text,
            severity(error.get("severity").unwrap_or(&serde_json::Value::Null))?,
            (start, end),
        ));
    }
    match code {
        Some(0) if !errors.is_empty() => Err(ParseError::shape(TOOL, "exit 0 with errors")),
        Some(1) if errors.is_empty() => Err(ParseError::shape(
            TOOL,
            "exit 1 without an error in the report",
        )),
        Some(2) if issues.is_empty() => Err(ParseError::shape(
            TOOL,
            "exit 2 without an issue in the report",
        )),
        _ => Ok(findings),
    }
}

fn preview(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .trim()
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[&str] = &["infra/a/main.tf", "infra/a/vars.tf"];
    const ISSUE: &str = r#"{"rule":{"name":"terraform_typed_variables","severity":"warning","link":"x"},"message":"`z` variable has no type","range":{"filename":"infra/a/vars.tf","start":{"line":3,"column":1},"end":{"line":3,"column":13}},"callers":[],"fixable":false,"fixed":false}"#;

    #[test]
    fn clean_run_is_clean() {
        let out = br#"{"issues":[],"errors":[]}"#;
        assert!(parse_tflint(out, Some(0), FILES).expect("clean").is_empty());
    }

    #[test]
    fn issues_map_rule_severity_and_span() {
        let out = format!(r#"{{"issues":[{ISSUE}],"errors":[]}}"#);
        let found = parse_tflint(out.as_bytes(), Some(2), FILES).expect("issues");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "infra/a/vars.tf");
        assert_eq!(found[0].finding.rule_id, "terraform_typed_variables");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.end.map(|end| end.column), Some(13));
    }

    #[test]
    fn info_and_error_severities_map() {
        for (raw, want) in [("info", ToolSeverity::Info), ("error", ToolSeverity::Error)] {
            let out = ISSUE.replace("warning", raw);
            let out = format!(r#"{{"issues":[{out}],"errors":[]}}"#);
            let found = parse_tflint(out.as_bytes(), Some(2), FILES).expect("issues");
            assert_eq!(found[0].finding.severity, want);
        }
    }

    #[test]
    fn hcl_diagnostics_are_errors_on_their_file() {
        let out = br#"{"issues":[],"errors":[{"summary":"Unclosed configuration block","message":"no closing brace","severity":"error","range":{"filename":"infra/a/main.tf","start":{"line":1,"column":18},"end":{"line":1,"column":19}}}]}"#;
        let found = parse_tflint(out, Some(1), FILES).expect("diagnostic");
        assert_eq!(found[0].file, "infra/a/main.tf");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert!(found[0].finding.message.contains("Unclosed"));
    }

    #[test]
    fn rangeless_errors_fail_the_run() {
        let out = br#"{"issues":[],"errors":[{"message":"Failed to initialize plugins; Plugin \"aws\" not found. Did you run \"tflint --init\"?","severity":"error"}]}"#;
        let err = parse_tflint(out, Some(1), FILES).expect_err("never clean");
        assert!(err.to_string().contains("tflint --init"), "{err}");
    }

    #[test]
    fn fails_closed_on_odd_output() {
        let ok = br#"{"issues":[],"errors":[]}"#;
        assert!(parse_tflint(ok, Some(1), FILES).is_err());
        assert!(parse_tflint(ok, Some(2), FILES).is_err());
        assert!(parse_tflint(ok, Some(3), FILES).is_err());
        assert!(parse_tflint(ok, None, FILES).is_err());
        assert!(parse_tflint(b"", Some(0), FILES).is_err());
        assert!(parse_tflint(b"not json", Some(0), FILES).is_err());
        assert!(parse_tflint(b"[]", Some(0), FILES).is_err());
        assert!(parse_tflint(&[0xff], Some(0), FILES).is_err());
        assert!(parse_tflint(br#"{"issues":[]}"#, Some(0), FILES).is_err());
    }

    #[test]
    fn rejects_unknown_files() {
        let out = format!(r#"{{"issues":[{ISSUE}],"errors":[]}}"#);
        let err = parse_tflint(out.as_bytes(), Some(2), &["other.tf"]).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
