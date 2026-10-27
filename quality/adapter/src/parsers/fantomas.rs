use crate::{ColumnEncoding, TextPosition};
use std::path::Path;

use serde::Deserialize;

use super::{
    check_output_size, code_name, finding, known, unformatted_at, FileFinding, ParseError,
};
use crate::ToolSeverity;

#[derive(Debug, Deserialize)]
struct FantomasReport {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    files: Vec<FantomasFile>,
}

#[derive(Debug, Deserialize)]
struct FantomasFile {
    path: String,
    status: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    diagnostics: Vec<FantomasDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct FantomasDiagnostic {
    code: String,
    message: String,
    range: FantomasRange,
}

#[derive(Debug, Deserialize)]
struct FantomasRange {
    #[serde(rename = "startLine")]
    start_line: u64,
    #[serde(rename = "startColumn")]
    start_column: u64,
}

pub fn parse_fantomas(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    root: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "fantomas";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if text.trim().is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with empty output", code_name(code)),
        });
    }
    let report: FantomasReport = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if let Some(error) = &report.error {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: error.clone(),
        });
    }
    let mut findings = Vec::new();
    let mut failed = false;
    for entry in &report.files {
        let named = entry.path.strip_prefix("./").unwrap_or(&entry.path);
        let relative = Path::new(named)
            .strip_prefix(root)
            .map_or_else(|_| named.to_owned(), dx_path::posix);
        let normalized = relative.as_str();
        match entry.status.as_str() {
            "unchanged" | "formatted" => {
                known(TOOL, files, normalized)?;
            }
            "needs-formatting" => {
                let checked = known(TOOL, files, normalized)?;
                findings.push(unformatted_at(TOOL, checked, 1, 1, ColumnEncoding::Scalars));
            }
            "error" => {
                let checked = known(TOOL, files, normalized)?;
                failed = true;
                if entry.diagnostics.is_empty() {
                    findings.push(finding(
                        TOOL,
                        checked,
                        "syntax".to_owned(),
                        entry.message.clone(),
                        ToolSeverity::Error,
                        TextPosition { line: 1, column: 1 },
                        ColumnEncoding::Scalars,
                    ));
                }
                for diagnostic in &entry.diagnostics {
                    findings.push(finding(
                        TOOL,
                        checked,
                        diagnostic.code.clone(),
                        diagnostic.message.clone(),
                        ToolSeverity::Error,
                        TextPosition {
                            line: diagnostic.range.start_line,
                            column: diagnostic.range.start_column,
                        },
                        ColumnEncoding::Scalars,
                    ));
                }
            }
            other => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("unknown file status: {other}"),
                });
            }
        }
    }
    if failed {
        if code != Some(1) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unparsable files with exit {} (want 1)", code_name(code)),
            });
        }
    } else if findings.is_empty() {
        if code != Some(0) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("exit {} with no needs-formatting files", code_name(code)),
            });
        }
    } else if code != Some(99) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "needs-formatting files with exit {} (want 99)",
                code_name(code)
            ),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = r#"{"files": [{"path": "fsharp/tests/fixtures/fantomas/Sample.fs", "status": "unchanged"}]}"#;
    const DIRTY: &str = r#"{"files": [{"path": "fsharp/tests/fixtures/fantomas/Sample.fs", "status": "needs-formatting"}]}"#;

    #[test]
    fn fantomas_reports_needs_formatting() {
        let findings = parse_fantomas(
            DIRTY.as_bytes(),
            Some(99),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
            Path::new("/scratch"),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "fsharp/tests/fixtures/fantomas/Sample.fs");
        let clean = parse_fantomas(
            CLEAN.as_bytes(),
            Some(0),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
            Path::new("/scratch"),
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_fantomas(b"", Some(99), &["x"], Path::new("/scratch")).is_err());
        assert!(parse_fantomas(
            DIRTY.as_bytes(),
            Some(0),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
            Path::new("/scratch")
        )
        .is_err());
        assert!(parse_fantomas(
            CLEAN.as_bytes(),
            Some(99),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
            Path::new("/scratch")
        )
        .is_err());
        assert!(parse_fantomas(
            DIRTY.as_bytes(),
            Some(99),
            &["other.fs"],
            Path::new("/scratch")
        )
        .is_err());
        assert!(parse_fantomas(&[0xff], Some(99), &["x"], Path::new("/scratch")).is_err());
        let syntax = r#"{"command": "check", "exitCode": 1, "error": null, "files": [{"path": "a.fs", "status": "error", "message": "a.fs could not be parsed by Fantomas", "diagnostics": [{"severity": "error", "code": "FS0010", "message": "Unexpected symbol", "range": {"startLine": 2, "startColumn": 9, "endLine": 2, "endColumn": 10}}]}]}"#;
        let found = parse_fantomas(syntax.as_bytes(), Some(1), &["a.fs"], Path::new("/scratch"))
            .expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.rule_id, "FS0010");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 9);
        assert!(
            parse_fantomas(syntax.as_bytes(), Some(0), &["a.fs"], Path::new("/scratch")).is_err()
        );
        let bare = r#"{"files": [{"path": "a.fs", "status": "error", "message": "bad"}]}"#;
        let found = parse_fantomas(bare.as_bytes(), Some(1), &["a.fs"], Path::new("/scratch"))
            .expect("parsed");
        assert_eq!(found[0].finding.rule_id, "syntax");
        let missing = r#"{"error": "Input path 'x.fs' not found.", "files": []}"#;
        assert!(parse_fantomas(
            missing.as_bytes(),
            Some(1),
            &["x.fs"],
            Path::new("/scratch")
        )
        .is_err());
        let absolute = DIRTY.replace(
            "fsharp/tests/fixtures/fantomas/Sample.fs",
            "/scratch/core/Sample.fs",
        );
        let found = parse_fantomas(
            absolute.as_bytes(),
            Some(99),
            &["core/Sample.fs"],
            Path::new("/scratch"),
        )
        .expect("parsed");
        assert_eq!(found[0].file, "core/Sample.fs");
        let bad_status = r#"{"files": [{"path": "x", "status": "bogus"}]}"#;
        assert!(parse_fantomas(
            bad_status.as_bytes(),
            Some(99),
            &["x"],
            Path::new("/scratch")
        )
        .is_err());
    }
}
