//! `terraform init -json` and `terraform validate -json` for one module directory.
//!
//! Both commands run with `-chdir=<dir>`, so a diagnostic names its file relative to that
//! directory (`main.tf`, or `../child/main.tf` for a sibling module). The parsers take the
//! absolute directory, join and normalise each name, and hold it to the known files.
//! Columns count Unicode characters.

use std::path::Path;

use super::{as_text, code_name, known_lexical, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "terraform";
const MESSAGE_LIMIT: usize = 300;

/// One diagnostic after the range check: located in a known file, or not located at all.
enum Diagnostic {
    Located(FileFinding),
    Unlocated { error: bool, text: String },
}

fn text_of(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn position(value: Option<&serde_json::Value>) -> TextPosition {
    let number = |key: &str| {
        value
            .and_then(|pos| pos.get(key))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .max(1)
    };
    TextPosition {
        line: number("line"),
        column: number("column"),
    }
}

fn diagnostic(
    diag: &serde_json::Value,
    rule: &str,
    dir: &Path,
    known: &[&str],
) -> Result<Diagnostic, ParseError> {
    let error = match diag.get("severity").and_then(serde_json::Value::as_str) {
        Some("error") => true,
        Some("warning") => false,
        other => {
            return Err(ParseError::shape(
                TOOL,
                format!("unknown diagnostic severity {other:?}"),
            ))
        }
    };
    let summary = text_of(diag, "summary");
    let detail = text_of(diag, "detail");
    let text = if detail.is_empty() {
        summary
    } else {
        format!("{summary}: {detail}")
    };
    let Some(range) = diag.get("range").filter(|range| !range.is_null()) else {
        return Ok(Diagnostic::Unlocated { error, text });
    };
    let name = range
        .get("filename")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, "range without filename"))?;
    let joined = dir.join(name);
    let file = known_lexical(TOOL, known, &joined.to_string_lossy())?;
    let start = position(range.get("start"));
    let end = range
        .get("end")
        .map(|end| position(Some(end)))
        .filter(|end| (end.line, end.column) > (start.line, start.column));
    Ok(Diagnostic::Located(FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message: text,
            severity: if error {
                ToolSeverity::Error
            } else {
                ToolSeverity::Warning
            },
            start,
            end,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }))
}

fn clip(text: &str) -> String {
    text.chars().take(MESSAGE_LIMIT).collect()
}

fn tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().rev().take(5).collect();
    clip(&lines.into_iter().rev().collect::<Vec<_>>().join(" | "))
}

/// Parses `terraform init -json` (JSON lines, Terraform 1.9 and later).
///
/// Only `diagnostic` lines matter. An error with a range is a finding (init reports HCL
/// syntax and configuration errors this way). A failing exit with no located error, such as a
/// provider missing from the mirror, fails the run so it is never read as clean.
pub fn parse_terraform_init(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    dir: &Path,
    known: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    let mut unlocated: Vec<String> = Vec::new();
    for line in as_text(TOOL, stdout)?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: format!("init -json line is not JSON ({err})"),
            })?;
        if value.get("type").and_then(serde_json::Value::as_str) != Some("diagnostic") {
            continue;
        }
        let diag = value
            .get("diagnostic")
            .ok_or_else(|| ParseError::shape(TOOL, "diagnostic line without diagnostic"))?;
        match diagnostic(diag, "init", dir, known)? {
            Diagnostic::Located(found) => {
                if found.finding.severity == ToolSeverity::Error {
                    findings.push(found);
                }
            }
            Diagnostic::Unlocated { error: true, text } => unlocated.push(text),
            Diagnostic::Unlocated { error: false, .. } => {}
        }
    }
    // After the located errors init adds one range-less summary ("Terraform encountered
    // problems during initialisation ..."), so range-less errors only matter when no located
    // error explains the failure.
    match code {
        Some(0) if unlocated.is_empty() => Ok(findings),
        Some(0) => Err(ParseError::shape(
            TOOL,
            format!("init exited 0 with errors: {}", clip(&unlocated.join("; "))),
        )),
        _ if !findings.is_empty() => Ok(findings),
        _ if !unlocated.is_empty() => Err(ParseError::shape(
            TOOL,
            format!("init failed: {}", clip(&unlocated.join("; "))),
        )),
        _ => Err(ParseError::shape(
            TOOL,
            format!("init exit {}: {}", code_name(code), tail(stderr)),
        )),
    }
}

/// Parses `terraform validate -json`: one object with `format_version`, `valid`,
/// `error_count` and `diagnostics`; exit 0 when valid and 1 otherwise.
///
/// The three signals must agree. A located diagnostic is a finding. A range-less error
/// (a failure outside any file) fails the run. A range-less warning is dropped: it has no
/// file to report on and does not change validity.
pub fn parse_terraform_validate(
    stdout: &[u8],
    code: Option<i32>,
    dir: &Path,
    known: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let report: serde_json::Value =
        serde_json::from_str(as_text(TOOL, stdout)?.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON object ({err})"),
        })?;
    let version = report
        .get("format_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, "missing format_version"))?;
    if version.split('.').next() != Some("1") {
        return Err(ParseError::shape(
            TOOL,
            format!("unsupported format_version {version}"),
        ));
    }
    let valid = report
        .get("valid")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| ParseError::shape(TOOL, "missing valid"))?;
    let error_count = report
        .get("error_count")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ParseError::shape(TOOL, "missing error_count"))?;
    let diagnostics = report
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing diagnostics"))?;
    let expected = if valid { 0 } else { 1 };
    if code != Some(expected) || valid != (error_count == 0) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} disagrees with valid={valid} error_count={error_count}",
                code_name(code)
            ),
        ));
    }
    let mut findings = Vec::new();
    let mut errors = 0_u64;
    for diag in diagnostics {
        match diagnostic(diag, "validate", dir, known)? {
            Diagnostic::Located(found) => {
                if found.finding.severity == ToolSeverity::Error {
                    errors += 1;
                }
                findings.push(found);
            }
            Diagnostic::Unlocated { error: true, text } => {
                return Err(ParseError::shape(
                    TOOL,
                    format!("validate failed outside any file: {}", clip(&text)),
                ));
            }
            Diagnostic::Unlocated { error: false, .. } => {}
        }
    }
    if errors != error_count {
        return Err(ParseError::shape(
            TOOL,
            format!("error_count {error_count} but {errors} located errors"),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: &[&str] = &["/s/main.tf", "/s/child/main.tf"];

    fn dir() -> &'static Path {
        Path::new("/s")
    }

    fn validate_json(valid: bool, count: u64, diags: &str) -> String {
        format!(
            r#"{{"format_version":"1.0","valid":{valid},"error_count":{count},"warning_count":0,"diagnostics":[{diags}]}}"#
        )
    }

    fn diag(severity: &str, summary: &str, file: Option<&str>) -> String {
        let range = file.map_or(String::new(), |file| {
            format!(
                r#","range":{{"filename":"{file}","start":{{"line":3,"column":5,"byte":9}},"end":{{"line":3,"column":9,"byte":13}}}}"#
            )
        });
        format!(r#"{{"severity":"{severity}","summary":"{summary}","detail":"more"{range}}}"#)
    }

    #[test]
    fn clean_validate_is_empty() {
        let out = validate_json(true, 0, "");
        let found = parse_terraform_validate(out.as_bytes(), Some(0), dir(), KNOWN).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn validate_error_maps_file_line_and_column() {
        let out = validate_json(
            false,
            1,
            &diag(
                "error",
                "Reference to undeclared input variable",
                Some("main.tf"),
            ),
        );
        let found = parse_terraform_validate(out.as_bytes(), Some(1), dir(), KNOWN).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/main.tf");
        assert_eq!(found[0].finding.start, TextPosition { line: 3, column: 5 });
        assert_eq!(
            found[0].finding.end,
            Some(TextPosition { line: 3, column: 9 })
        );
        assert_eq!(found[0].finding.rule_id, "validate");
        assert!(found[0]
            .finding
            .message
            .starts_with("Reference to undeclared"));
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Scalars);
    }

    #[test]
    fn unsupported_argument_on_terraform_data() {
        let out = validate_json(
            false,
            1,
            &diag("error", "Unsupported argument", Some("main.tf")),
        );
        let found = parse_terraform_validate(out.as_bytes(), Some(1), dir(), KNOWN).unwrap();
        assert_eq!(found[0].finding.message, "Unsupported argument: more");
    }

    #[test]
    fn parent_relative_filename_is_normalised() {
        let out = validate_json(false, 1, &diag("error", "bad", Some("../child/main.tf")));
        let found = parse_terraform_validate(out.as_bytes(), Some(1), Path::new("/s/other"), KNOWN)
            .unwrap();
        assert_eq!(found[0].file, "/s/child/main.tf");
    }

    #[test]
    fn unknown_file_is_rejected() {
        let out = validate_json(false, 1, &diag("error", "bad", Some("nope.tf")));
        assert!(matches!(
            parse_terraform_validate(out.as_bytes(), Some(1), dir(), KNOWN),
            Err(ParseError::UnknownFile { .. })
        ));
    }

    #[test]
    fn rangeless_error_fails_the_run() {
        let out = validate_json(false, 1, &diag("error", "Module not installed", None));
        assert!(parse_terraform_validate(out.as_bytes(), Some(1), dir(), KNOWN).is_err());
    }

    #[test]
    fn rangeless_warning_is_dropped() {
        let out = validate_json(true, 0, &diag("warning", "Deprecated", None));
        let found = parse_terraform_validate(out.as_bytes(), Some(0), dir(), KNOWN).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn exit_code_must_agree_with_valid() {
        let out = validate_json(true, 0, "");
        assert!(parse_terraform_validate(out.as_bytes(), Some(1), dir(), KNOWN).is_err());
        let out = validate_json(false, 1, &diag("error", "bad", Some("main.tf")));
        assert!(parse_terraform_validate(out.as_bytes(), Some(0), dir(), KNOWN).is_err());
        assert!(parse_terraform_validate(out.as_bytes(), None, dir(), KNOWN).is_err());
    }

    #[test]
    fn unknown_major_version_is_rejected() {
        let out = validate_json(true, 0, "").replace("\"1.0\"", "\"2.0\"");
        assert!(parse_terraform_validate(out.as_bytes(), Some(0), dir(), KNOWN).is_err());
    }

    #[test]
    fn non_json_validate_output_is_rejected() {
        assert!(parse_terraform_validate(b"Success!", Some(0), dir(), KNOWN).is_err());
    }

    fn init_line(summary: &str, file: Option<&str>) -> String {
        let range = file.map_or(String::new(), |file| {
            format!(
                r#","range":{{"filename":"{file}","start":{{"line":2,"column":1,"byte":1}},"end":{{"line":2,"column":2,"byte":2}}}}"#
            )
        });
        format!(
            r#"{{"@level":"error","@message":"Error: {summary}","type":"diagnostic","diagnostic":{{"severity":"error","summary":"{summary}","detail":"d"{range}}}}}"#
        )
    }

    #[test]
    fn clean_init_ignores_other_line_types() {
        let out = r#"{"@level":"info","@message":"Terraform has been successfully initialized!","type":"init_output"}"#;
        let found = parse_terraform_init(out.as_bytes(), b"", Some(0), dir(), KNOWN).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn init_syntax_error_is_a_finding() {
        let out = init_line("Argument or block definition required", Some("main.tf"));
        let found = parse_terraform_init(out.as_bytes(), b"", Some(1), dir(), KNOWN).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/main.tf");
        assert_eq!(found[0].finding.rule_id, "init");
    }

    #[test]
    fn init_syntax_error_ignores_the_rangeless_summary_that_follows() {
        let out = format!(
            "{}\n{}\n",
            init_line("Invalid expression", Some("main.tf")),
            init_line("Terraform encountered problems during initialisation", None)
        );
        let found = parse_terraform_init(out.as_bytes(), b"", Some(1), dir(), KNOWN).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn init_missing_provider_fails_the_run() {
        let out = init_line("Failed to query available provider packages", None);
        let err = parse_terraform_init(out.as_bytes(), b"", Some(1), dir(), KNOWN).unwrap_err();
        assert!(err
            .to_string()
            .contains("Failed to query available provider"));
    }

    #[test]
    fn init_failure_without_diagnostics_reports_stderr() {
        let err = parse_terraform_init(b"", b"boom\n", Some(1), dir(), KNOWN).unwrap_err();
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn init_non_json_line_is_rejected() {
        assert!(parse_terraform_init(b"not json\n", b"", Some(0), dir(), KNOWN).is_err());
    }
}
