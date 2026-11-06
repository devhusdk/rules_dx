//! Shared reader for GNU-style compiler diagnostics (`file:line:col: severity: message`).
//!
//! Compiler-specific parsers supply the severity table and the encoding of the
//! columns. Lines that are not diagnostic headers (source excerpts, carets,
//! notes, flang `because:` lines) are skipped.

use super::{FileFinding, ParseError, as_text, finding, require_findings};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

/// What a compiler-specific parser does with one severity word.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Disposition {
    Report(ToolSeverity),
    /// Continuation lines such as `note:` that belong to an earlier header.
    Skip,
}

pub(super) struct Dialect {
    pub tool: &'static str,
    pub encoding: ColumnEncoding,
    pub severity: fn(&str) -> Option<Disposition>,
    /// Splits a trailing `[-Wflag]` style rule id off the message.
    pub bracket_rule: bool,
}

struct Header<'a> {
    file: &'a str,
    line: u64,
    column: u64,
    severity: &'a str,
    message: &'a str,
}

fn header(line: &str) -> Option<Header<'_>> {
    let skip = if dx_path::drive_rooted(line) { 2 } else { 0 };
    let mut from = skip;
    while let Some(at) = line[from..].find(':') {
        let colon = from + at;
        from = colon + 1;
        let rest = &line[colon + 1..];
        let (line_text, rest) = rest.split_once(':')?;
        let Ok(line_no) = line_text.parse::<u64>() else {
            continue;
        };
        let (column, rest) = match rest.split_once(':') {
            Some((col, after)) if col.parse::<u64>().is_ok() => (col.parse().ok()?, after),
            _ => (1, rest),
        };
        let rest = rest.strip_prefix(' ')?;
        let (severity, message) = rest.split_once(':')?;
        if severity.is_empty()
            || !severity
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == ' ')
            || severity.starts_with(' ')
            || severity.ends_with(' ')
        {
            continue;
        }
        return Some(Header {
            file: &line[..colon],
            line: line_no,
            column,
            severity,
            message: message.trim(),
        });
    }
    None
}

/// Reads every diagnostic header. `files` are the checked sources. A warning in
/// a file outside them is dropped, since a dependency's warnings are not this
/// target's. An error in such a file is a failure naming the file, so a broken
/// dependency can never look clean.
pub(super) fn read(
    dialect: &Dialect,
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let tool = dialect.tool;
    let mut findings = Vec::new();
    for raw in as_text(tool, output)?.lines() {
        let Some(head) = header(raw.trim()) else {
            continue;
        };
        let Some(disposition) = (dialect.severity)(head.severity.to_ascii_lowercase().as_str())
        else {
            continue;
        };
        let Disposition::Report(severity) = disposition else {
            continue;
        };
        if head.message.is_empty() {
            return Err(ParseError::shape(tool, format!("malformed message: {raw}")));
        }
        let name = head.file.strip_prefix("./").unwrap_or(head.file);
        let owned = files.iter().copied().find(|file| *file == name);
        let Some(file) = owned else {
            if severity == ToolSeverity::Error {
                return Err(ParseError::UnknownFile {
                    tool,
                    path: name.to_owned(),
                });
            }
            continue;
        };
        let (message, rule_id) = match head.message.rfind(" [") {
            Some(start) if dialect.bracket_rule && head.message.ends_with(']') => (
                head.message[..start].trim_end(),
                head.message[start + 2..head.message.len() - 1].to_owned(),
            ),
            _ => (head.message, String::new()),
        };
        findings.push(finding(
            tool,
            file,
            rule_id,
            message.to_owned(),
            severity,
            TextPosition {
                line: head.line,
                column: head.column,
            },
            dialect.encoding,
        ));
    }
    require_findings(tool, &findings, code)?;
    // A failed compile must report an error; warnings alone cannot explain it.
    if code != Some(0)
        && !findings
            .iter()
            .any(|f| f.finding.severity == ToolSeverity::Error)
    {
        return Err(ParseError::shape(
            tool,
            "nonzero exit without a parsed error diagnostic".to_owned(),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "src/a.c";

    fn word(word: &str) -> Option<Disposition> {
        match word {
            "error" => Some(Disposition::Report(ToolSeverity::Error)),
            "warning" => Some(Disposition::Report(ToolSeverity::Warning)),
            "note" => Some(Disposition::Skip),
            _ => None,
        }
    }

    const DIALECT: Dialect = Dialect {
        tool: "gnu",
        encoding: ColumnEncoding::Utf8,
        severity: word,
        bracket_rule: true,
    };

    fn run(text: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        read(&DIALECT, text.as_bytes(), Some(code), &[FILE])
    }

    #[test]
    fn header_reads_file_line_column_severity_and_message() {
        let head = header("src/a.c:3:7: error: bad thing").expect("header");
        assert_eq!(head.file, "src/a.c");
        assert_eq!(head.line, 3);
        assert_eq!(head.column, 7);
        assert_eq!(head.severity, "error");
        assert_eq!(head.message, "bad thing");
    }

    #[test]
    fn header_defaults_a_missing_column_to_one() {
        let head = header("src/a.c:3: warning: odd").expect("header");
        assert_eq!(head.line, 3);
        assert_eq!(head.column, 1);
    }

    #[test]
    fn header_keeps_a_drive_letter_in_the_file() {
        let head = header("C:\\src\\a.c:3:7: error: bad").expect("header");
        assert_eq!(head.file, "C:\\src\\a.c");
        assert_eq!(head.line, 3);
    }

    #[test]
    fn header_rejects_lines_that_are_not_diagnostics() {
        assert!(header("    int x = ;").is_none());
        assert!(header("        ^").is_none());
        assert!(header("src/a.c:x:y: error: bad").is_none());
        assert!(header("src/a.c:3:7: 12: bad").is_none());
    }

    #[test]
    fn read_reports_errors_and_warnings_with_their_positions() {
        let text = "src/a.c:3:7: error: bad\nsrc/a.c:4:1: warning: odd\n";
        let found = run(text, 1).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 7);
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn read_splits_a_trailing_bracket_into_the_rule() {
        let found = run("src/a.c:1:1: warning: unused [-Wunused]\n", 0).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "-Wunused");
        assert_eq!(found[0].finding.message, "unused");
        let plain = Dialect {
            bracket_rule: false,
            ..DIALECT
        };
        let text = "src/a.c:1:1: warning: unused [-Wunused]\n";
        let found = read(&plain, text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "");
        assert_eq!(found[0].finding.message, "unused [-Wunused]");
    }

    #[test]
    fn read_skips_notes_unknown_words_and_strips_dot_slash() {
        let text =
            "./src/a.c:2:1: error: bad\nsrc/a.c:2:1: note: see here\nsrc/a.c:2:1: remark: hm\n";
        let found = run(text, 1).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
    }

    #[test]
    fn read_drops_foreign_warnings_but_fails_on_foreign_errors() {
        let text = "dep/b.c:1:1: warning: noise\nsrc/a.c:1:1: error: bad\n";
        assert_eq!(run(text, 1).expect("parsed").len(), 1);
        let dep = "dep/b.c:1:1: error: broken\n";
        assert!(matches!(run(dep, 1), Err(ParseError::UnknownFile { .. })));
    }

    #[test]
    fn read_fails_closed() {
        assert!(run("", 0).expect("clean").is_empty());
        assert!(run("", 1).is_err());
        assert!(read(&DIALECT, b"", None, &[FILE]).is_err());
        assert!(run("src/a.c:1:1: warning: odd\n", 1).is_err());
        assert!(run("src/a.c:1:1: error: \n", 1).is_err());
        assert!(read(&DIALECT, &[0xff], Some(1), &[FILE]).is_err());
    }
}
