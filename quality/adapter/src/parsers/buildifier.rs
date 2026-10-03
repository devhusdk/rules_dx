use serde::Deserialize;

use super::{check_output_size, finding, known, unformatted_at, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct BuildifierReport {
    files: Vec<BuildifierFile>,
}

#[derive(Debug, Deserialize)]
struct BuildifierFile {
    filename: String,
    formatted: bool,
    valid: bool,
    warnings: Vec<BuildifierWarning>,
}

#[derive(Debug, Deserialize)]
struct BuildifierWarning {
    start: BuildifierPosition,
    end: BuildifierPosition,
    category: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct BuildifierPosition {
    line: u64,
    column: u64,
}

pub fn parse_buildifier(
    stdout: &[u8],
    stderr: &[u8],
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "buildifier";
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let report: BuildifierReport = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let stderr_text = String::from_utf8_lossy(stderr);
    let mut findings = Vec::new();
    for file in &report.files {
        let checked = known(TOOL, files, &file.filename)?;
        if !file.valid {
            let (line, column, message) = buildifier_syntax(&stderr_text, &file.filename);
            findings.push(finding(
                TOOL,
                checked,
                String::new(),
                message,
                ToolSeverity::Error,
                line,
                column,
            ));
            continue;
        }
        if !file.formatted {
            findings.push(unformatted_at(TOOL, checked, 1, 1));
        }
        for warning in &file.warnings {
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: warning.category.clone(),
                    message: warning.message.clone(),
                    severity: ToolSeverity::Warning,
                    start: TextPosition {
                        line: warning.start.line,
                        column: warning.start.column,
                    },
                    end: Some(TextPosition {
                        line: warning.end.line,
                        column: warning.end.column,
                    }),
                    suggestions: Vec::new(),
                },
            });
        }
    }
    Ok(findings)
}

fn buildifier_syntax(stderr: &str, filename: &str) -> (u64, u64, String) {
    let prefix = format!("{filename}:");
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            let mut parts = rest.splitn(3, ':');
            if let (Some(line_text), Some(column_text), Some(message)) =
                (parts.next(), parts.next(), parts.next())
            {
                if let (Ok(line), Ok(column)) =
                    (line_text.parse::<u64>(), column_text.parse::<u64>())
                {
                    if line >= 1 && column >= 1 {
                        let message = message.trim().to_owned();
                        let message = if message.is_empty() {
                            "syntax error".to_owned()
                        } else {
                            message
                        };
                        return (line, column, message);
                    }
                }
            }
        }
    }
    (1, 1, "syntax error".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILDIFIER_DIRTY: &str = r#"{"success":false,"files":[{"filename":"/s/dirty.bzl","formatted":false,"valid":true,"warnings":[{"start":{"line":1,"column":1},"end":{"line":1,"column":2},"category":"module-docstring","actionable":true,"autoFixable":false,"message":"The file has no module docstring.","url":"https://example.com"}]}]}"#;

    const BUILDIFIER_CLEAN: &str = r#"{"success":true,"files":[{"filename":"/s/clean.bzl","formatted":true,"valid":true,"warnings":[]}]}"#;

    const BUILDIFIER_BROKEN: &str = r#"{"success":false,"files":[{"filename":"/s/broken.bzl","formatted":false,"valid":false,"warnings":[]}]}"#;

    #[test]
    fn buildifier_reports_format_and_warnings() {
        let findings =
            parse_buildifier(BUILDIFIER_DIRTY.as_bytes(), b"", &["/s/dirty.bzl"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/dirty.bzl");
        assert_eq!(findings[0].finding.rule_id, "");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].finding.rule_id, "module-docstring");
        assert_eq!(
            (findings[1].finding.start, findings[1].finding.end),
            (
                TextPosition { line: 1, column: 1 },
                Some(TextPosition { line: 1, column: 2 })
            )
        );
        assert!(
            parse_buildifier(BUILDIFIER_CLEAN.as_bytes(), b"", &["/s/clean.bzl"])
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn buildifier_positions_syntax_errors_from_stderr() {
        let findings = parse_buildifier(
            BUILDIFIER_BROKEN.as_bytes(),
            b"/s/broken.bzl:3:1: syntax error",
            &["/s/broken.bzl"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 3, column: 1 }, None)
        );
        assert_eq!(findings[0].finding.message, "syntax error");
        assert!(parse_buildifier(b"not json", b"crash", &["/s/broken.bzl"]).is_err());
        assert!(parse_buildifier(BUILDIFIER_CLEAN.as_bytes(), b"", &["/s/other.bzl"]).is_err());
    }

    #[test]
    fn buildifier_rejects_bytes_and_positions_outside_the_grammar() {
        assert!(parse_buildifier(b"\xff\xfe", b"", &["/s/x.bzl"]).is_err());
        let broken = r#"{"success":false,"files":[{"filename":"/s/broken.bzl","formatted":false,"valid":false,"warnings":[]}]}"#;
        let fallback = parse_buildifier(
            broken.as_bytes(),
            b"/s/broken.bzl:0:5: bad line\nnoise without prefix\n/s/broken.bzl:a:b: bad numbers\n/s/broken.bzl:nocolons",
            &["/s/broken.bzl"],
        )
        .expect("parsed");
        assert_eq!(fallback.len(), 1);
        assert_eq!(
            (fallback[0].finding.start, fallback[0].finding.end),
            (TextPosition { line: 1, column: 1 }, None)
        );
        let empty = parse_buildifier(broken.as_bytes(), b"/s/broken.bzl:3:1:", &["/s/broken.bzl"])
            .expect("parsed");
        assert_eq!(empty[0].finding.message, "syntax error");
        assert_eq!(empty[0].finding.start, TextPosition { line: 3, column: 1 });
    }
}
