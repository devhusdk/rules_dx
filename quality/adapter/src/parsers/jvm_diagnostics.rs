use serde::Deserialize;

use super::{check_output_size, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

/// The only log version this parser understands.
const VERSION: u64 = 1;

/// javac reports this error, with no position, when `-Werror` turned warnings into a failure.
const WARNINGS_AS_ERRORS: &str = "compiler.err.warnings.and.werror";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    version: u64,
    tool: String,
    exit: i32,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    file: String,
    line: u64,
    column: u64,
    severity: String,
    code: String,
    message: String,
}

/// Parses the versioned JSON log of a check-only compiler driver (javac today).
///
/// `code` is the driver's own exit status. The compiler's status is inside the log. Findings
/// outside `files` (generated sources, unlocated notes) are dropped, but a failed compile that
/// leaves no error among the kept findings is an execution error, never a clean result.
pub fn parse_jvm_diagnostics(
    tool: &'static str,
    bytes: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, bytes)?;
    if code != Some(0) {
        return Err(ParseError::shape(
            tool,
            format!("driver exit {}", super::code_name(code)),
        ));
    }
    let log: Log = serde_json::from_slice(bytes).map_err(|err| ParseError::Json {
        tool,
        detail: err.to_string(),
    })?;
    if log.version != VERSION || log.tool != tool {
        return Err(ParseError::shape(
            tool,
            format!("log version {} tool {:?}", log.version, log.tool),
        ));
    }
    let promote = log
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == WARNINGS_AS_ERRORS);
    let mut findings = Vec::new();
    let mut dropped: Option<String> = None;
    for diagnostic in log.diagnostics {
        let severity = match diagnostic.severity.as_str() {
            "error" => ToolSeverity::Error,
            "warning" if promote => ToolSeverity::Error,
            "warning" => ToolSeverity::Warning,
            "note" => ToolSeverity::Info,
            other => {
                return Err(ParseError::shape(
                    tool,
                    format!("unknown severity {other:?}"),
                ));
            }
        };
        let owned = diagnostic.line > 0
            && diagnostic.column > 0
            && files.contains(&diagnostic.file.as_str());
        if !owned {
            if matches!(severity, ToolSeverity::Error) && dropped.is_none() {
                dropped = Some(format!(
                    "{}: {} ({})",
                    diagnostic.file, diagnostic.message, diagnostic.code
                ));
            }
            continue;
        }
        findings.push(FileFinding {
            file: diagnostic.file,
            finding: Finding {
                tool_id: tool.to_owned(),
                rule_id: diagnostic.code,
                message: diagnostic.message,
                severity,
                start: TextPosition {
                    line: diagnostic.line,
                    column: diagnostic.column,
                },
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Utf16,
                related: Vec::new(),
            },
        });
    }
    let failed = findings
        .iter()
        .any(|found| matches!(found.finding.severity, ToolSeverity::Error));
    if log.exit != 0 && !failed {
        let detail = dropped.map_or_else(
            || format!("javac exit {} with no error diagnostic", log.exit),
            |first| {
                format!(
                    "javac exit {} with no error in a checked file: {first}",
                    log.exit
                )
            },
        );
        return Err(ParseError::shape(tool, detail));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(exit: i32, diagnostics: &str) -> Vec<u8> {
        format!(r#"{{"version":1,"tool":"javac","exit":{exit},"diagnostics":[{diagnostics}]}}"#)
            .into_bytes()
    }

    fn entry(file: &str, line: u64, severity: &str, code: &str) -> String {
        format!(
            r#"{{"file":"{file}","line":{line},"column":{},"severity":"{severity}","code":"{code}","message":"m"}}"#,
            if line == 0 { 0 } else { 3 }
        )
    }

    #[test]
    fn maps_severity_code_and_position() {
        let body = [
            entry("j/A.java", 4, "error", "compiler.err.x"),
            entry("j/A.java", 5, "warning", "compiler.warn.y"),
            entry("j/A.java", 6, "note", "compiler.note.z"),
        ]
        .join(",");
        let found = parse_jvm_diagnostics("javac", &log(1, &body), Some(0), &["j/A.java"]).unwrap();
        let shape: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.finding.start.line,
                    f.finding.rule_id.as_str(),
                    f.finding.severity,
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                (4, "compiler.err.x", ToolSeverity::Error),
                (5, "compiler.warn.y", ToolSeverity::Warning),
                (6, "compiler.note.z", ToolSeverity::Info),
            ]
        );
        assert_eq!(found[0].finding.start.column, 3);
        assert_eq!(found[0].finding.tool_id, "javac");
    }

    #[test]
    fn werror_promotes_warnings_to_errors() {
        let body = [
            entry("j/A.java", 5, "warning", "compiler.warn.y"),
            entry("", 0, "error", WARNINGS_AS_ERRORS),
        ]
        .join(",");
        let found = parse_jvm_diagnostics("javac", &log(1, &body), Some(0), &["j/A.java"]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn clean_log_is_clean_and_other_files_are_dropped() {
        assert!(parse_jvm_diagnostics("javac", &log(0, ""), Some(0), &[])
            .unwrap()
            .is_empty());
        let body = entry("gen/B.java", 2, "warning", "compiler.warn.y");
        assert!(
            parse_jvm_diagnostics("javac", &log(0, &body), Some(0), &["j/A.java"])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_compile_with_nothing_kept_is_never_clean() {
        let body = entry("gen/B.java", 2, "error", "compiler.err.q");
        let err = parse_jvm_diagnostics("javac", &log(1, &body), Some(0), &["j/A.java"])
            .expect_err("dropped error")
            .to_string();
        assert!(
            err.contains("gen/B.java") && err.contains("compiler.err.q"),
            "{err}"
        );
        assert!(parse_jvm_diagnostics("javac", &log(1, ""), Some(0), &[]).is_err());
    }

    #[test]
    fn malformed_and_foreign_logs_fail() {
        assert!(parse_jvm_diagnostics("javac", b"not json", Some(0), &[]).is_err());
        assert!(parse_jvm_diagnostics("javac", b"", Some(0), &[]).is_err());
        assert!(parse_jvm_diagnostics("javac", &log(0, ""), Some(1), &[]).is_err());
        let v2 = br#"{"version":2,"tool":"javac","exit":0,"diagnostics":[]}"#;
        assert!(parse_jvm_diagnostics("javac", v2, Some(0), &[]).is_err());
        let other = br#"{"version":1,"tool":"kotlinc","exit":0,"diagnostics":[]}"#;
        assert!(parse_jvm_diagnostics("javac", other, Some(0), &[]).is_err());
        let sev = entry("j/A.java", 1, "fatal", "c");
        assert!(parse_jvm_diagnostics("javac", &log(0, &sev), Some(0), &["j/A.java"]).is_err());
    }
}
