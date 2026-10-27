use crate::{ColumnEncoding, TextPosition};
use std::path::Path;

use super::{as_text, code_name, finding, known_relative, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;

const TOOL: &str = "mix_format";

const UNFORMATTED_HEADER: &str = "The following files are not formatted:";
const FAILED_PREFIX: &str = "mix format failed for file: ";

/// Findings from `mix format --check-formatted`: listed unformatted files, or the one file it could not parse.
pub fn parse_mix_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    if !as_text(TOOL, stdout)?.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "--check-formatted wrote to stdout"));
    }
    let raw = as_text(TOOL, stderr)?;
    let lines: Vec<(bool, String)> = raw
        .lines()
        .map(|line| (line.starts_with('\u{1b}'), strip_ansi(line)))
        .collect();
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut listing = false;
    for (index, (styled, line)) in lines.iter().enumerate() {
        if line.trim() == UNFORMATTED_HEADER {
            listing = true;
            continue;
        }
        if let Some(path) = line.strip_prefix(FAILED_PREFIX) {
            let file = known_relative(TOOL, files, cwd, path.trim())?;
            findings.push(syntax_error(file, path.trim(), &lines[index + 1..]));
            listing = false;
            continue;
        }
        if listing && is_listed_path(*styled, line) {
            let file = known_relative(TOOL, files, cwd, line.trim())?;
            if !findings.iter().any(|found| found.file == file) {
                findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
            }
        }
    }
    let shaped = match code {
        Some(0) => findings.is_empty() && raw.trim().is_empty(),
        Some(1) => !findings.is_empty(),
        _ => false,
    };
    if !shaped {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} findings", code_name(code), findings.len()),
        ));
    }
    Ok(findings)
}

fn is_listed_path(styled: bool, line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    styled || !(line.starts_with(char::is_whitespace) || trimmed.contains('|'))
}

fn syntax_error(file: &str, reported: &str, rest: &[(bool, String)]) -> FileFinding {
    let header = rest
        .iter()
        .map(|(_, line)| line.trim())
        .find(|line| line.starts_with("** ("));
    let (line, column) = header
        .and_then(|header| position(header, reported))
        .unwrap_or((1, 1));
    let message = rest
        .iter()
        .take_while(|(_, line)| !line.starts_with(FAILED_PREFIX))
        .find_map(|(_, line)| line.trim().strip_prefix("error: "))
        .map(str::to_owned)
        .or_else(|| header.map(|header| summary(header, reported)))
        .unwrap_or_else(|| "could not parse the file".to_owned());
    finding(
        TOOL,
        file,
        "syntax".to_owned(),
        message,
        ToolSeverity::Error,
        TextPosition { line, column },
        ColumnEncoding::Scalars,
    )
}

/// The line and column from a `** (Kind) ... on <path>:<line>:<column>:` header.
fn position(header: &str, reported: &str) -> Option<(u64, u64)> {
    let marker = format!(" on {reported}:");
    let at = header.rfind(&marker)?;
    let mut parts = header[at + marker.len()..].split(':');
    let line = parts.next()?.parse().ok()?;
    let column = parts.next().and_then(|part| part.parse().ok()).unwrap_or(1);
    Some((line, column))
}

fn summary(header: &str, reported: &str) -> String {
    let body = header
        .split_once(") ")
        .map_or(header, |(_, rest)| rest)
        .trim();
    let marker = format!(" on {reported}:");
    body.rfind(&marker)
        .map_or(body, |at| &body[..at])
        .to_owned()
}

pub(super) fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const CWD: &str = "/s";
    const DIRTY: &str = "** (Mix) mix format failed due to --check-formatted.\nThe following files are not formatted:\n\n\u{1b}[1m\u{1b}[31m/s/b.ex\n\u{1b}[0m\n1 1  |defmodule A do\n2   -|  def f( x ),do: x\n  2 +|  def f(x), do: x\n3 3  |end\n\n\u{1b}[1m\u{1b}[31m/s/lib/c.ex\n\u{1b}[0m\n1   -|x=1\n  1 +|x = 1\n\n";
    const MISMATCHED: &str = "mix format failed for file: syn.ex\n** (MismatchedDelimiterError) mismatched delimiter found on syn.ex:3:1:\n    error: unexpected reserved word: end\n    \u{2502}\n  2 \u{2502}   def f(x, do: x\n    \u{2514}\u{2500} syn.ex:3:1\n    (elixir 1.20.4) lib/code.ex:1123: Code.format_string!/2\n";
    const MISSING_TOKEN: &str = "mix format failed for file: lib/d.ex\n** (TokenMissingError) token missing on lib/d.ex:3:6:\n    error: missing terminator: end\n";
    const BARE: &str =
        "mix format failed for file: lib/d.ex\n** (SyntaxError) invalid syntax found on lib/d.ex:2:20:\n";

    fn cwd() -> &'static Path {
        Path::new(CWD)
    }

    #[test]
    fn mix_format_reports_each_listed_file_once() {
        let files = ["/s/a.ex", "/s/b.ex", "/s/lib/c.ex"];
        let findings =
            parse_mix_format(b"", DIRTY.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        let named: Vec<&str> = findings.iter().map(|found| found.file.as_str()).collect();
        assert_eq!(named, ["/s/b.ex", "/s/lib/c.ex"]);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let twice = DIRTY.replace("/s/lib/c.ex", "/s/b.ex");
        assert_eq!(
            parse_mix_format(b"", twice.as_bytes(), Some(1), &files, cwd())
                .expect("parsed")
                .len(),
            1
        );
    }

    #[test]
    fn mix_format_lists_paths_without_ansi_styling() {
        let files = ["/s/b.ex"];
        let plain = "** (Mix) mix format failed due to --check-formatted.\nThe following files are not formatted:\n\n/s/b.ex\n\n1   -|x=1\n  1 +|x = 1\n";
        let findings =
            parse_mix_format(b"", plain.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/b.ex");
    }

    #[test]
    fn mix_format_locates_a_parse_failure() {
        let files = ["/s/syn.ex"];
        let findings =
            parse_mix_format(b"", MISMATCHED.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings.len(), 1);
        let error = &findings[0].finding;
        assert_eq!(findings[0].file, "/s/syn.ex");
        assert_eq!(error.rule_id, "syntax");
        assert_eq!(error.severity, ToolSeverity::Error);
        assert_eq!(error.message, "unexpected reserved word: end");
        assert_eq!(error.start, TextPosition { line: 3, column: 1 });

        let files = ["/s/lib/d.ex"];
        let missing = parse_mix_format(b"", MISSING_TOKEN.as_bytes(), Some(1), &files, cwd())
            .expect("parsed");
        assert_eq!(missing[0].finding.message, "missing terminator: end");
        assert_eq!(
            missing[0].finding.start,
            TextPosition { line: 3, column: 6 }
        );
    }

    #[test]
    fn mix_format_falls_back_to_the_exception_summary() {
        let files = ["/s/lib/d.ex"];
        let findings =
            parse_mix_format(b"", BARE.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].finding.message, "invalid syntax found");
        assert_eq!(
            findings[0].finding.start,
            TextPosition {
                line: 2,
                column: 20
            }
        );
        let headless = "mix format failed for file: lib/d.ex\n";
        let findings =
            parse_mix_format(b"", headless.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].finding.message, "could not parse the file");
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 1, column: 1 }
        );
    }

    #[test]
    fn mix_format_rejects_unshaped_output() {
        let files = ["/s/b.ex"];
        assert!(parse_mix_format(b"", b"", Some(0), &files, cwd())
            .expect("parsed")
            .is_empty());
        assert!(parse_mix_format(
            b"",
            DIRTY.as_bytes(),
            Some(0),
            &["/s/b.ex", "/s/lib/c.ex"],
            cwd()
        )
        .is_err());
        assert!(parse_mix_format(b"", b"", Some(1), &files, cwd()).is_err());
        assert!(parse_mix_format(b"", b"noise\n", Some(0), &files, cwd()).is_err());
        assert!(parse_mix_format(
            b"",
            b"** (Mix) Could not find a file to format.\n",
            Some(1),
            &files,
            cwd()
        )
        .is_err());
        assert!(parse_mix_format(b"", b"", None, &files, cwd()).is_err());
        assert!(parse_mix_format(b"", b"", Some(2), &files, cwd()).is_err());
        assert!(parse_mix_format(b"x\n", b"", Some(0), &files, cwd()).is_err());
        assert!(parse_mix_format(b"", &[0xff], Some(1), &files, cwd()).is_err());
        assert!(matches!(
            parse_mix_format(b"", DIRTY.as_bytes(), Some(1), &files, cwd()),
            Err(ParseError::UnknownFile { path, .. }) if path == "/s/lib/c.ex"
        ));
        assert!(matches!(
            parse_mix_format(b"", MISMATCHED.as_bytes(), Some(1), &files, cwd()),
            Err(ParseError::UnknownFile { path, .. }) if path == "syn.ex"
        ));
    }

    #[test]
    fn strip_ansi_removes_only_escape_sequences() {
        assert_eq!(strip_ansi("\u{1b}[1m\u{1b}[31m/s/b.ex\u{1b}[0m"), "/s/b.ex");
        assert_eq!(strip_ansi("plain [1m"), "plain [1m");
        assert_eq!(strip_ansi("\u{1b}x"), "\u{1b}x");
    }
}
