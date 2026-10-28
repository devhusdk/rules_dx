//! Shared reader for GNU-style compiler diagnostics (`file:line:col: severity: message`).
//!
//! Compiler-specific parsers supply the severity table and the encoding of the
//! columns. Lines that are not diagnostic headers (source excerpts, carets,
//! notes, flang `because:` lines) are skipped.

use super::{as_text, finding, require_findings, FileFinding, ParseError};
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
