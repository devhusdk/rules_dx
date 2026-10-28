use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, columned, finding, prefixed, FileFinding, ParseError};

const TOOL: &str = "zig_ast_check";

/// Findings from `zig ast-check <file>`: `path:line:col: error:` lines on stderr.
///
/// Fails closed: exit 1 without a diagnostic for a passed file is a usage or IO failure
/// (`unable to open file`), never a clean run. `note:` lines are context, not findings.
pub fn parse_zig_ast_check(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !as_text(TOOL, stdout)?.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "unexpected stdout output"));
    }
    let errors = zig_errors(TOOL, stderr, files, "ast_check")?;
    match code {
        Some(0) if errors.is_empty() => Ok(errors),
        Some(0) => Err(ParseError::shape(
            TOOL,
            "exit 0 with error diagnostics (want 1)",
        )),
        Some(1) if !errors.is_empty() => Ok(errors),
        Some(1) => {
            let first = as_text(TOOL, stderr)?
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("no output");
            Err(ParseError::shape(
                TOOL,
                format!("exit 1 without a diagnostic for a checked file: {first}"),
            ))
        }
        other => Err(ParseError::shape(
            TOOL,
            format!("exit {} (want 0 or 1)", code_name(other)),
        )),
    }
}

/// `path:line:col: error: message` lines from Zig's stderr. Columns are UTF-8 bytes.
pub(super) fn zig_errors(
    tool: &'static str,
    stderr: &[u8],
    files: &[&str],
    rule_id: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(tool, stderr)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((file, rest)) = prefixed(line, files) else {
            continue;
        };
        let Some((row, col, tail)) = columned(tool, line, rest).ok() else {
            continue;
        };
        if let Some(message) = tail.strip_prefix("error:") {
            out.push(finding(
                tool,
                file,
                rule_id.to_owned(),
                message.trim().to_owned(),
                ToolSeverity::Error,
                TextPosition {
                    line: row,
                    column: col,
                },
                ColumnEncoding::Utf8,
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 1] = ["lib/a.zig"];

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_zig_ast_check(b"", b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn errors_come_in_order_and_notes_are_skipped() {
        let err = "lib/a.zig:2:9: error: unused local variable\n    const x = 1;\n        ^\n\
                   lib/a.zig:5:3: error: redeclaration of 'y'\n  y;\n  ^\n\
                   lib/a.zig:4:3: note: declared here\n";
        let found = parse_zig_ast_check(b"", err.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].file, "lib/a.zig");
        assert_eq!(found[0].finding.rule_id, "ast_check");
        assert_eq!(found[0].finding.message, "unused local variable");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(found[1].finding.start.line, 5);
    }

    #[test]
    fn columns_are_utf8_bytes() {
        let err = "lib/a.zig:1:31: error: bad\n";
        let found = parse_zig_ast_check(b"", err.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Utf8);
    }

    #[test]
    fn failures_are_errors() {
        let io = b"error: unable to open file 'x' for ast-check: FileNotFound\n";
        assert!(parse_zig_ast_check(b"", io, Some(1), &FILES).is_err());
        let diag = b"lib/a.zig:1:1: error: bad\n";
        assert!(parse_zig_ast_check(b"", diag, Some(0), &FILES).is_err());
        assert!(parse_zig_ast_check(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_zig_ast_check(b"", b"", None, &FILES).is_err());
        let other = b"lib/b.zig:1:1: error: bad\n";
        assert!(parse_zig_ast_check(b"", other, Some(1), &FILES).is_err());
        assert!(parse_zig_ast_check(b"", &[0xff], Some(1), &FILES).is_err());
        assert!(parse_zig_ast_check(b"junk", b"", Some(0), &FILES).is_err());
    }
}
