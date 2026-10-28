use serde::Deserialize;

use super::{
    as_text, check_output_size, code_name, finding, known_native, unformatted_at, FileFinding,
    ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "opa";
const PARSE_RULE: &str = "rego_parse_error";

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(default)]
    errors: Vec<Problem>,
}

#[derive(Debug, Deserialize)]
struct Problem {
    message: String,
    code: String,
    location: Option<Location>,
}

#[derive(Debug, Deserialize)]
struct Location {
    file: String,
    row: u64,
    col: u64,
}

/// `opa fmt FILE` prints the formatted source to stdout and exits 0, or exits 2 and writes
/// `FILE:ROW: rego_parse_error: MESSAGE` lines to stderr when the file does not parse. The first
/// differing line is reported; a parse error is an error finding. Anything else is an error,
/// never clean. A CRLF file differs from the LF output on every line, so it is reported.
pub fn parse_opa_fmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    if code == Some(0) {
        if !errors.trim().is_empty() {
            return Err(ParseError::shape(TOOL, "exit 0 with stderr output"));
        }
        if stdout.is_empty() && !input.is_empty() {
            return Err(ParseError::shape(TOOL, "empty output for a non-empty file"));
        }
        if stdout == input {
            return Ok(Vec::new());
        }
        let line = stdout
            .split(|b| *b == b'\n')
            .zip(input.split(|b| *b == b'\n'))
            .position(|(formatted, original)| formatted != original)
            .map_or_else(
                || {
                    stdout
                        .split(|b| *b == b'\n')
                        .count()
                        .min(input.split(|b| *b == b'\n').count())
                },
                |index| index + 1,
            );
        return Ok(vec![unformatted_at(
            TOOL,
            file,
            line as u64,
            1,
            ColumnEncoding::Scalars,
        )]);
    }
    if code.is_none() {
        return Err(ParseError::shape(TOOL, "killed by a signal"));
    }
    let prefix = format!("{file}:");
    let mut findings = Vec::new();
    for line in errors.lines() {
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let Some((row, rest)) = rest.split_once(": ") else {
            continue;
        };
        let Ok(row) = row.parse::<u64>() else {
            continue;
        };
        let Some(message) = rest.strip_prefix(&format!("{PARSE_RULE}: ")) else {
            continue;
        };
        findings.push(finding(
            TOOL,
            file,
            PARSE_RULE.to_owned(),
            message.to_owned(),
            ToolSeverity::Error,
            TextPosition {
                line: row,
                column: 1,
            },
            ColumnEncoding::Scalars,
        ));
    }
    if findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} without a located parse error", code_name(code)),
        ));
    }
    Ok(findings)
}

/// `opa check --format json` exits 0 and prints nothing when clean. On errors it exits 1 and
/// writes `{"errors":[{message, code, location:{file, row, col}}]}` to stderr, not stdout. Columns
/// count Unicode scalar values from 1. Any other exit, output on a clean run, non-JSON output, an
/// empty error list or an error without a location is an error, never clean.
pub fn parse_opa_check(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    if !out.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "unexpected stdout output"));
    }
    match code {
        Some(0) => {
            if err.trim().is_empty() {
                Ok(Vec::new())
            } else {
                Err(ParseError::shape(TOOL, "exit 0 with stderr output"))
            }
        }
        Some(1) => {
            let report: Report = serde_json::from_str(err).map_err(|source| ParseError::Json {
                tool: TOOL,
                detail: source.to_string(),
            })?;
            if report.errors.is_empty() {
                return Err(ParseError::shape(TOOL, "exit 1 without errors"));
            }
            let mut findings = Vec::with_capacity(report.errors.len());
            for problem in report.errors {
                let Some(location) = problem.location else {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("{} without a location: {}", problem.code, problem.message),
                    ));
                };
                let file = known_native(TOOL, files, &location.file)?;
                findings.push(finding(
                    TOOL,
                    file,
                    problem.code,
                    problem.message,
                    ToolSeverity::Error,
                    TextPosition {
                        line: location.row,
                        column: location.col,
                    },
                    ColumnEncoding::Scalars,
                ));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/a.rego", "/s/b.rego"];
    const BROKEN: &str = r#"{"errors":[{"message":"undefined function data.lib.nope","code":"rego_type_error","location":{"file":"/s/b.rego","row":3,"col":6}}]}"#;

    #[test]
    fn fmt_clean_and_dirty() {
        let clean = b"package a\n\nx := 1\n";
        assert!(parse_opa_fmt(clean, b"", Some(0), clean, "/s/a.rego")
            .expect("ok")
            .is_empty());
        let dirty = b"package a\n\nx :=   1\n";
        let found = parse_opa_fmt(clean, b"", Some(0), dirty, "/s/a.rego").expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.message, "file is not formatted");
    }

    #[test]
    fn fmt_crlf_input_is_reported() {
        let found =
            parse_opa_fmt(b"package a\n", b"", Some(0), b"package a\r\n", "/s/a.rego").expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.start.line, 1);
    }

    #[test]
    fn fmt_parse_error_is_a_finding() {
        let stderr = b"failed to format Rego source file: 2 errors occurred:\n/s/a.rego:2: rego_parse_error: unexpected assign token\n\tv := := 1\n\t     ^\n";
        let found = parse_opa_fmt(b"", stderr, Some(2), b"package x\nv := := 1\n", "/s/a.rego")
            .expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.rule_id, "rego_parse_error");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn fmt_anything_else_is_an_error() {
        let src = b"package a\n";
        assert!(parse_opa_fmt(b"", b"", Some(0), src, "/s/a.rego").is_err());
        assert!(parse_opa_fmt(src, b"noise\n", Some(0), src, "/s/a.rego").is_err());
        assert!(parse_opa_fmt(b"", b"boom\n", Some(2), src, "/s/a.rego").is_err());
        assert!(parse_opa_fmt(b"", b"", Some(1), src, "/s/a.rego").is_err());
        assert!(parse_opa_fmt(b"", b"", None, src, "/s/a.rego").is_err());
        let other = b"/s/b.rego:2: rego_parse_error: x\n";
        assert!(parse_opa_fmt(b"", other, Some(2), src, "/s/a.rego").is_err());
        assert!(parse_opa_fmt(&[0xff], b"", Some(0), src, "/s/a.rego").is_ok());
        assert!(parse_opa_fmt(b"", &[0xff], Some(2), src, "/s/a.rego").is_err());
    }

    #[test]
    fn check_clean_run_has_no_findings() {
        assert!(parse_opa_check(b"", b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn check_reports_errors_from_stderr() {
        let found = parse_opa_check(b"", BROKEN.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/b.rego");
        assert_eq!(found[0].finding.rule_id, "rego_type_error");
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 6);
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Scalars);
    }

    #[test]
    fn check_anything_else_is_an_error() {
        assert!(parse_opa_check(b"", b"noise\n", Some(0), &FILES).is_err());
        assert!(parse_opa_check(BROKEN.as_bytes(), b"", Some(1), &FILES).is_err());
        assert!(parse_opa_check(BROKEN.as_bytes(), BROKEN.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_opa_check(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_opa_check(b"", b"not json\n", Some(1), &FILES).is_err());
        assert!(parse_opa_check(b"", br#"{"errors":[]}"#, Some(1), &FILES).is_err());
        assert!(parse_opa_check(b"", BROKEN.as_bytes(), Some(2), &FILES).is_err());
        assert!(parse_opa_check(b"", BROKEN.as_bytes(), None, &FILES).is_err());
        let stranger = BROKEN.replace("/s/b.rego", "/s/other.rego");
        assert!(parse_opa_check(b"", stranger.as_bytes(), Some(1), &FILES).is_err());
        let unlocated = r#"{"errors":[{"message":"m","code":"rego_compile_error"}]}"#;
        assert!(parse_opa_check(b"", unlocated.as_bytes(), Some(1), &FILES).is_err());
    }
}
