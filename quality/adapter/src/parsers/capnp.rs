use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "capnp";
const CRASH_MARKER: &str = "*** Uncaught exception ***";

struct Located<'a> {
    path: &'a str,
    line: u64,
    column: u64,
    severity: ToolSeverity,
    message: &'a str,
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Reads `path:line:col[-col]: error|warning: message`.
fn locate(line: &str) -> Option<Located<'_>> {
    for (kind, severity) in [
        (": error: ", ToolSeverity::Error),
        (": warning: ", ToolSeverity::Warning),
    ] {
        let Some((head, message)) = line.split_once(kind) else {
            continue;
        };
        let (rest, span) = head.rsplit_once(':')?;
        let (path, row) = rest.rsplit_once(':')?;
        let column = span.split_once('-').map_or(span, |(start, _)| start);
        return Some(Located {
            path,
            line: number(row)?,
            column: number(column)?,
            severity,
            message,
        });
    }
    None
}

/// The first `failed:` line after an uncaught-exception banner, as a message.
fn crash_message(stderr: &str) -> Option<String> {
    let mut lines = stderr.lines();
    lines.find(|line| line.trim() == CRASH_MARKER)?;
    let line = lines.next()?;
    let (_, failed) = line.split_once(": failed: ")?;
    let message = failed.split_once("; ").map_or(failed, |(_, text)| text);
    let message = message.trim();
    (!message.is_empty()).then(|| message.to_owned())
}

fn finding(
    file: &str,
    severity: ToolSeverity,
    line: u64,
    column: u64,
    message: &str,
) -> FileFinding {
    let rule = if severity == ToolSeverity::Error {
        "error"
    } else {
        "warning"
    };
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message: message.to_owned(),
            severity,
            start: TextPosition {
                line,
                column: column.max(1),
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Utf8,
            related: Vec::new(),
        },
    }
}

/// Parses the stderr of one `capnp compile -o-` run over the checked files.
///
/// The CodeGeneratorRequest on stdout is binary and never read. Findings in
/// imported schemas belong to their own targets, so only the checked files
/// report here. Output the grammar cannot read is never clean: exit 0 allows
/// warnings only, exit 1 needs an error in a checked file (or the compiler's
/// uncaught-exception banner, reported on the first checked file), and
/// anything else, including a signal, is a shape error.
pub fn parse_capnp(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stderr)?;
    let err = String::from_utf8_lossy(stderr);
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    if code == Some(1) {
        if let Some(message) = crash_message(&err) {
            let Some(file) = files.first() else {
                return Err(ParseError::shape(TOOL, "no checked file".to_owned()));
            };
            return Ok(vec![finding(file, ToolSeverity::Error, 1, 1, &message)]);
        }
    }
    let mut findings = Vec::new();
    let mut errors = 0usize;
    let mut own_errors = 0usize;
    for line in err.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(found) = locate(trimmed) else {
            return Err(ParseError::shape(
                TOOL,
                format!("unreadable line: {trimmed}"),
            ));
        };
        if found.message.is_empty() || found.line == 0 {
            return Err(ParseError::shape(
                TOOL,
                format!("malformed finding: {trimmed}"),
            ));
        }
        if found.severity == ToolSeverity::Error {
            errors += 1;
        }
        let Some(file) = files.iter().find(|file| **file == found.path) else {
            continue;
        };
        if found.severity == ToolSeverity::Error {
            own_errors += 1;
        }
        // capnp appends a freshly generated random ID; drop it so output is stable.
        let message = if found.message.starts_with("File does not declare an ID.") {
            "File does not declare an ID; generate one with `capnp id`"
        } else {
            found.message
        };
        findings.push(finding(
            file,
            found.severity,
            found.line,
            found.column,
            message,
        ));
    }
    match code {
        Some(0) if errors > 0 => Err(ParseError::shape(
            TOOL,
            format!("{errors} error(s) with exit 0"),
        )),
        Some(1) if own_errors == 0 => Err(ParseError::shape(
            TOOL,
            "exit 1 with no error finding in the checked files".to_owned(),
        )),
        _ => Ok(findings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "pkg/a.capnp";

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_capnp(b"", Some(0), &[FILE])
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn error_with_column_span_is_a_finding() {
        let err = b"pkg/a.capnp:2:18-22: error: Not defined: Nope\n";
        let found = parse_capnp(err, Some(1), &[FILE]).expect("findings");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.message, "Not defined: Nope");
        assert_eq!(
            found[0].finding.start,
            TextPosition {
                line: 2,
                column: 18
            }
        );
    }

    #[test]
    fn related_note_in_another_file_is_not_this_files_finding() {
        let err = b"pkg/dup.capnp:1:1: error: Duplicate ID @0xdbb9ad1f14bf0b36.\n\
pkg/base.capnp:1:1: error: ID @0xdbb9ad1f14bf0b36 originally used here.\n";
        let found = parse_capnp(err, Some(1), &["pkg/dup.capnp"]).expect("findings");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "pkg/dup.capnp");
    }

    #[test]
    fn the_random_generated_id_is_not_part_of_the_message() {
        let err = b"pkg/a.capnp:1:1: error: File does not declare an ID.  I've generated one for you.  Add this line to your file: @0xfacc81fbf872dc19;\n";
        let found = parse_capnp(err, Some(1), &[FILE]).expect("findings");
        assert_eq!(
            found[0].finding.message,
            "File does not declare an ID; generate one with `capnp id`"
        );
    }

    #[test]
    fn warning_with_exit_zero_is_a_finding() {
        let err = b"pkg/a.capnp:3:5: warning: something\n";
        let found = parse_capnp(err, Some(0), &[FILE]).expect("warning");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn uncaught_exception_is_reported_on_the_first_file() {
        let err = b"*** Uncaught exception ***\n\
kj/filesystem.c++:315: failed: expected parts.size() > 0 [0 > 0]; can't use \"..\" to break out of starting directory\n\
stack: 1 2 3\n";
        let found = parse_capnp(err, Some(1), &[FILE]).expect("crash");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].finding.message,
            "can't use \"..\" to break out of starting directory"
        );
    }

    #[test]
    fn unreadable_output_is_never_clean() {
        assert!(parse_capnp(b"", Some(1), &[FILE]).is_err());
        assert!(parse_capnp(b"", None, &[FILE]).is_err());
        assert!(parse_capnp(b"", Some(2), &[FILE]).is_err());
        assert!(parse_capnp(b"garbage\n", Some(0), &[FILE]).is_err());
        assert!(parse_capnp(b"garbage\n", Some(1), &[FILE]).is_err());
        let err = b"pkg/a.capnp:1:2: error: m\n";
        assert!(parse_capnp(err, Some(0), &[FILE]).is_err());
        let other = b"pkg/other.capnp:1:2: error: m\n";
        assert!(parse_capnp(other, Some(1), &[FILE]).is_err());
        assert!(parse_capnp(b"\xff\xfe\n", Some(0), &[FILE]).is_err());
    }
}
