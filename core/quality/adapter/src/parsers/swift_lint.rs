use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{FileFinding, ParseError, as_text, code_name, finding};

const TOOL: &str = "swift_lint";

/// Findings from one batched `swift-format lint --strict` run.
///
/// Every stderr line must be `file:LINE:COL: error: message` for one of `files`. A message that
/// starts with `[Rule]` is a style finding named by that rule; any other message is a syntax
/// error. Exit 0 with lines, exit 1 without lines and any other exit are errors, never a clean run.
pub fn parse_swift_lint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !as_text(TOOL, stdout)?.trim().is_empty() {
        return Err(shape("unexpected stdout".to_owned()));
    }
    let text = as_text(TOOL, stderr)?;
    let mut out = Vec::new();
    for line in text.lines() {
        out.push(diagnostic(line, files)?);
    }
    match (code, out.is_empty()) {
        (Some(0), true) | (Some(1), false) => Ok(out),
        (Some(0), false) => Err(shape(format!(
            "exit 0 with {} diagnostic lines (want none)",
            out.len()
        ))),
        (Some(1), true) => Err(shape("exit 1 without a diagnostic line".to_owned())),
        _ => Err(shape(format!("exit {} (want 0 or 1)", code_name(code)))),
    }
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

fn diagnostic(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let bad = |detail: &str| shape(format!("{detail}: {line}"));
    let file = files
        .iter()
        .copied()
        .find(|file| {
            line.strip_prefix(file)
                .is_some_and(|rest| rest.starts_with(':'))
        })
        .ok_or_else(|| ParseError::UnknownFile {
            tool: TOOL,
            path: line.split(':').next().unwrap_or(line).to_owned(),
        })?;
    let rest = &line[file.len() + 1..];
    let mut parts = rest.splitn(3, ':');
    let (Some(row), Some(col), Some(tail)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(bad("line outside swift-format's grammar"));
    };
    let (Ok(row), Ok(col)) = (row.parse::<u64>(), col.parse::<u64>()) else {
        return Err(bad("position is not a number"));
    };
    if row == 0 || col == 0 {
        return Err(bad("position is not 1-based"));
    }
    let message = tail
        .trim()
        .strip_prefix("error:")
        .ok_or_else(|| bad("unknown severity"))?
        .trim();
    let (rule, severity, message) = match message
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
    {
        Some((rule, text)) if !rule.is_empty() => {
            (rule.to_owned(), ToolSeverity::Warning, text.to_owned())
        }
        _ => ("syntax".to_owned(), ToolSeverity::Error, message.to_owned()),
    };
    Ok(finding(
        TOOL,
        file,
        rule,
        message,
        severity,
        TextPosition {
            line: row,
            column: col,
        },
        ColumnEncoding::Utf8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/a.swift", "/s/b.swift"];

    #[test]
    fn swift_lint_is_clean_on_exit_zero_without_lines() {
        assert!(
            parse_swift_lint(b"", b"", Some(0), &FILES)
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn swift_lint_names_style_findings_by_rule() {
        let err = b"/s/a.swift:2:13: error: [DoNotUseSemicolons] remove ';'\n";
        let found = parse_swift_lint(b"", err, Some(1), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/a.swift");
        assert_eq!(found[0].finding.rule_id, "DoNotUseSemicolons");
        assert_eq!(found[0].finding.message, "remove ';'");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 13);
    }

    #[test]
    fn swift_lint_reports_syntax_errors() {
        let err = b"/s/b.swift:1:6: error: expected identifier in function\n";
        let found = parse_swift_lint(b"", err, Some(1), &FILES).expect("parsed");
        assert_eq!(found[0].file, "/s/b.swift");
        assert_eq!(found[0].finding.rule_id, "syntax");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn swift_lint_fails_closed_on_anything_it_does_not_know() {
        let ok = b"/s/a.swift:1:1: error: [Spacing] add 1 space\n";
        assert!(parse_swift_lint(b"", ok, Some(0), &FILES).is_err());
        assert!(parse_swift_lint(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_swift_lint(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_swift_lint(b"", b"", None, &FILES).is_err());
        assert!(parse_swift_lint(b"junk", b"", Some(0), &FILES).is_err());
        let other = b"/s/c.swift:1:1: error: [Spacing] add 1 space\n";
        assert!(parse_swift_lint(b"", other, Some(1), &FILES).is_err());
        let zero = b"/s/a.swift:0:1: error: [Spacing] add 1 space\n";
        assert!(parse_swift_lint(b"", zero, Some(1), &FILES).is_err());
        let warn = b"/s/a.swift:1:1: warning: [Spacing] add 1 space\n";
        assert!(parse_swift_lint(b"", warn, Some(1), &FILES).is_err());
        assert!(parse_swift_lint(b"", &[0xff], Some(1), &FILES).is_err());
    }
}
