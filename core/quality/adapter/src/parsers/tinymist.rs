use super::{FileFinding, ParseError, as_text, code_name, finding, known_spelled};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

const TOOL: &str = "tinymist";
const RULE_ID: &str = "tinymist/lint";

/// Findings from one `tinymist lint --diagnostic-format short` run over a single file.
///
/// Every diagnostic is a `<path>:<line>:<col>: <severity>: <message>` line on stderr, and the
/// tool exits 1 when it printed any. Diagnostics for files outside `files` (modules the file
/// imports) are dropped, but they still count as output. Exit 0 with output, exit 1 without
/// output, any other exit and any line outside the format are errors, never a clean run.
pub fn parse_tinymist(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stdout = as_text(TOOL, stdout)?;
    let stderr = as_text(TOOL, stderr)?;
    if stdout.contains('\u{1b}') || stderr.contains('\u{1b}') {
        return Err(ParseError::shape(TOOL, "unexpected ANSI escape in output"));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut lines = 0;
    for line in stdout.lines().chain(stderr.lines()) {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        lines += 1;
        let diagnostic = split_diagnostic(line).ok_or_else(|| {
            ParseError::shape(TOOL, format!("line outside the short format: {line}"))
        })?;
        let Ok(file) = known_spelled(TOOL, files, diagnostic.path) else {
            continue;
        };
        let found = finding(
            TOOL,
            file,
            RULE_ID.to_owned(),
            diagnostic.message.to_owned(),
            diagnostic.severity,
            TextPosition {
                line: diagnostic.line,
                column: diagnostic.column,
            },
            ColumnEncoding::Scalars,
        );
        if !findings.contains(&found) {
            findings.push(found);
        }
    }
    match (code, lines) {
        (Some(0), 0) | (Some(1), 1..) => Ok(findings),
        (Some(0), _) => Err(ParseError::shape(TOOL, "exit 0 with diagnostics")),
        (Some(1), 0) => Err(ParseError::shape(TOOL, "exit 1 without diagnostics")),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} (want 0 or 1)", code_name(code)),
        )),
    }
}

struct Diagnostic<'a> {
    path: &'a str,
    line: u64,
    column: u64,
    severity: ToolSeverity,
    message: &'a str,
}

/// Splits at the first `:<line>:<col>: <severity>: ` so drive letters and colons in the message
/// are left alone. The line is 1-based and the column a 0-based count of characters.
fn split_diagnostic(text: &str) -> Option<Diagnostic<'_>> {
    text.match_indices(':').find_map(|(at, _)| {
        let rest = &text[at + 1..];
        let (line, rest) = rest.split_once(':')?;
        let (column, rest) = rest.split_once(": ")?;
        let (severity, message) = rest.split_once(": ")?;
        let severity = match severity {
            "error" => ToolSeverity::Error,
            "warning" => ToolSeverity::Warning,
            "hint" | "info" | "note" => ToolSeverity::Info,
            _ => return None,
        };
        let line: u64 = line.parse().ok().filter(|value| *value > 0)?;
        let column: u64 = column.parse::<u64>().ok()?.checked_add(1)?;
        Some(Diagnostic {
            path: &text[..at],
            line,
            column,
            severity,
            message,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 1] = ["/s/a.typ"];

    fn parse(stderr: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_tinymist(b"", stderr.as_bytes(), Some(code), &FILES)
    }

    #[test]
    fn tinymist_clean_is_empty() {
        assert!(parse("", 0).expect("parsed").is_empty());
    }

    #[test]
    fn tinymist_reports_errors_and_warnings_once() {
        let findings = parse(
            "/s/a.typ:2:0: error: unknown variable: x\n/s/a.typ:6:16: warning: unknown font family: nope\n/s/a.typ:6:16: warning: unknown font family: nope\n",
            1,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.typ");
        assert_eq!(findings[0].finding.rule_id, "tinymist/lint");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.message, "unknown variable: x");
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 2, column: 1 }
        );
        assert_eq!(findings[1].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn tinymist_accepts_windows_paths() {
        let files = ["C:\\s\\a.typ"];
        let out = parse_tinymist(
            b"",
            b"C:\\s\\a.typ:1:2: error: bad: thing\n",
            Some(1),
            &files,
        )
        .expect("parsed");
        assert_eq!(out[0].finding.message, "bad: thing");
    }

    #[test]
    fn tinymist_drops_files_that_were_not_passed() {
        let out = parse("/s/lib.typ:1:1: error: bad\n", 1).expect("parsed");
        assert!(out.is_empty());
    }

    #[test]
    fn tinymist_fails_closed() {
        assert!(parse("", 1).is_err());
        assert!(parse("/s/a.typ:1:1: error: bad\n", 0).is_err());
        assert!(parse("", 2).is_err());
        assert!(parse_tinymist(b"", b"", None, &FILES).is_err());
        assert!(parse("error: file not found (searched at /s/b.typ)\n", 1).is_err());
        assert!(parse("/s/a.typ:0:0: error: bad\n", 1).is_err());
        assert!(parse("\u{1b}[31m/s/a.typ:1:1: error: bad\n", 1).is_err());
    }
}
