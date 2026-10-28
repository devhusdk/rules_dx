use serde::Deserialize;

use super::{
    as_text, check_output_size, code_name, finding, known_native, unformatted_at, FileFinding,
    ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "stylua";

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    mismatches: Vec<Mismatch>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Mismatch {
    original_start_line: u64,
}

#[derive(Debug, Deserialize)]
struct SyntaxError {
    filename: String,
    location: Location,
    message: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct Location {
    start_line: u64,
    start_column: u64,
}

/// `--check --output-format=json` writes one JSON object per unformatted file to stdout and exits 1.
/// Files that do not parse are never listed there: they exit 0 and put a JSON array of located
/// errors on stderr, so stderr is read whatever the exit code. Any other exit (2 for a missing
/// file or bad config) or any output outside those two shapes is an error, never clean.
pub fn parse_stylua(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let listed = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    let mut findings = Vec::new();
    let mut mismatched = false;
    for line in listed.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let report: Report = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        if let Some(kind) = &report.kind {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "{kind}: {}",
                    report.message.as_deref().unwrap_or("no message")
                ),
            ));
        }
        let Some(path) = report.file.as_deref() else {
            return Err(ParseError::shape(TOOL, "report without a file"));
        };
        let file = known_native(TOOL, files, path)?;
        if report.mismatches.is_empty() {
            return Err(ParseError::shape(TOOL, "report without mismatches"));
        }
        mismatched = true;
        let first = report
            .mismatches
            .iter()
            .map(|m| m.original_start_line)
            .min()
            .unwrap_or(0);
        findings.push(unformatted_at(
            TOOL,
            file,
            first + 1,
            1,
            ColumnEncoding::Scalars,
        ));
    }
    let parse_errors = syntax_errors(errors, files)?;
    let failed = !parse_errors.is_empty();
    findings.extend(parse_errors);
    match code {
        Some(1) if mismatched => Ok(findings),
        Some(0) if !mismatched => Ok(findings),
        _ => Err(ParseError::shape(
            TOOL,
            format!(
                "unexpected exit {} with {}",
                code_name(code),
                if failed || mismatched {
                    "diagnostics"
                } else {
                    "no diagnostics"
                }
            ),
        )),
    }
}

fn syntax_errors(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    if stderr.trim().is_empty() {
        return Ok(findings);
    }
    for batch in serde_json::Deserializer::from_str(stderr).into_iter::<Vec<SyntaxError>>() {
        let batch = batch.map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        for error in batch {
            let file = known_native(TOOL, files, &error.filename)?;
            findings.push(finding(
                TOOL,
                file,
                error.kind,
                error.message,
                ToolSeverity::Error,
                TextPosition {
                    line: error.location.start_line,
                    column: error.location.start_column,
                },
                ColumnEncoding::Scalars,
            ));
        }
    }
    if findings.is_empty() {
        return Err(ParseError::shape(TOOL, "stderr without located errors"));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["a.lua", "b.lua", "bad.lua"];
    const DIRTY: &str = r#"{"file":"b.lua","mismatches":[{"expected":"x\n","expected_end_line":0,"expected_start_line":0,"original":"y\n","original_end_line":2,"original_start_line":2}]}"#;
    const BROKEN: &str = r#"[{"filename":"bad.lua","location":{"end":7,"end_column":8,"end_line":1,"start":6,"start_column":7,"start_line":3},"message":"unexpected token `=`","type":"parse_error"}]"#;

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_stylua(b"", b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn mismatch_reports_the_first_line() {
        let findings = parse_stylua(DIRTY.as_bytes(), b"", Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "b.lua");
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn syntax_errors_exit_zero_but_are_findings() {
        let findings = parse_stylua(b"", BROKEN.as_bytes(), Some(0), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "bad.lua");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 7);
    }

    #[test]
    fn mismatches_and_syntax_errors_mix() {
        let findings =
            parse_stylua(DIRTY.as_bytes(), BROKEN.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn anything_else_is_an_error() {
        let tool_error =
            br#"{"message":"no file or directory found matching 'x.lua'","type":"error"}"#;
        assert!(parse_stylua(tool_error, b"", Some(2), &FILES).is_err());
        assert!(parse_stylua(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_stylua(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_stylua(b"", b"", None, &FILES).is_err());
        assert!(parse_stylua(DIRTY.as_bytes(), b"", Some(0), &FILES).is_err());
        assert!(parse_stylua(b"garbage\n", b"", Some(1), &FILES).is_err());
        assert!(parse_stylua(b"", b"warning: boom\n", Some(0), &FILES).is_err());
        let stranger = DIRTY.replace("b.lua", "other.lua");
        assert!(parse_stylua(stranger.as_bytes(), b"", Some(1), &FILES).is_err());
    }
}
