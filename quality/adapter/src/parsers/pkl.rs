use super::{as_text, code_name, finding, known_native, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "pkl";
const TRAILER: &str = "An error occurred during formatting.";

/// Exit 0 is clean, 11 lists unformatted files, and 1 lists them and reports located errors.
pub fn parse_pkl(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let listed = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    let mut findings = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for line in listed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let file = known_native(TOOL, files, line)?;
        if seen.contains(&file) {
            continue;
        }
        seen.push(file);
        findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
    }
    match code {
        Some(0) if findings.is_empty() && errors.trim().is_empty() => Ok(findings),
        Some(11) if !findings.is_empty() && errors.trim().is_empty() => Ok(findings),
        Some(1) => {
            let before = findings.len();
            findings.extend(format_errors(errors, files)?);
            if findings.len() == before {
                return Err(ParseError::shape(TOOL, "exit 1 with no located errors"));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} with output", code_name(code)),
        )),
    }
}

fn format_errors(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    let mut closed = false;
    for line in stderr.lines().map(str::trim_end).filter(|l| !l.is_empty()) {
        if closed {
            return Err(ParseError::shape(
                TOOL,
                format!("output after the closing line: {line}"),
            ));
        }
        if line == TRAILER {
            closed = true;
            continue;
        }
        let Some((path, message)) = line
            .strip_prefix("Could not format `")
            .and_then(|rest| rest.split_once("`: "))
        else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognized error output: {line}"),
            ));
        };
        let file = known_native(TOOL, files, path)?;
        let (message, row, column) = split_location(message);
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.to_owned(),
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    if !closed {
        return Err(ParseError::shape(
            TOOL,
            "error output without the closing line",
        ));
    }
    Ok(findings)
}

/// The message and the start of its trailing ` at (<row>:<col> - <row>:<col>)` range.
fn split_location(message: &str) -> (&str, u64, u64) {
    let located: Option<(&str, u64, u64)> = message.strip_suffix(')').and_then(|rest| {
        let (text, range) = rest.rsplit_once(" at (")?;
        let (start, _) = range.split_once(" - ")?;
        let (row, column) = start.split_once(':')?;
        Some((text, row.parse().ok()?, column.parse().ok()?))
    });
    match located {
        Some((text, row, column)) => (text.trim_end_matches('.'), row.max(1), column.max(1)),
        None => (message, 1, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 4] = ["/s/main.pkl", "/s/lib/a.pkl", "/s/bad.pkl", "/s/open.pkl"];
    const BROKEN: &str = "Could not format `/s/bad.pkl`: Unexpected token `=`. at (1:5 - 1:6)\nCould not format `/s/open.pkl`: org.pkl.parser.ParserError: Missing `\"` delimiter.\nAn error occurred during formatting.\n";

    #[test]
    fn pkl_lists_unformatted_files() {
        let findings =
            parse_pkl(b"/s/main.pkl\n/s/lib/a.pkl\n", b"", Some(11), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/main.pkl");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert!(parse_pkl(b"", b"", Some(0), &FILES).expect("ok").is_empty());
    }

    #[test]
    fn pkl_locates_errors_next_to_unformatted_files() {
        let findings =
            parse_pkl(b"/s/lib/a.pkl\n", BROKEN.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[1].file, "/s/bad.pkl");
        assert_eq!(findings[1].finding.message, "Unexpected token `=`");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].finding.start.line, 1);
        assert_eq!(findings[1].finding.start.column, 5);
        assert_eq!(findings[2].file, "/s/open.pkl");
        assert_eq!(findings[2].finding.start.column, 1);
    }

    #[test]
    fn pkl_never_reports_a_failure_as_clean() {
        assert!(parse_pkl(b"", b"", Some(11), &FILES).is_err());
        assert!(parse_pkl(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_pkl(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_pkl(b"", b"", None, &FILES).is_err());
        assert!(parse_pkl(b"/s/main.pkl\n", b"", Some(0), &FILES).is_err());
        assert!(parse_pkl(b"", b"warning\n", Some(0), &FILES).is_err());
        assert!(parse_pkl(&[0xff], b"", Some(0), &FILES).is_err());
        let crash = b"An unexpected error has occurred. Would you mind filing a bug report?\n\njava.lang.IndexOutOfBoundsException\n";
        assert!(parse_pkl(b"", crash, Some(1), &FILES).is_err());
        let io = b"IO error while reading `/s/gone.pkl`: /s/gone.pkl\nAn error occurred during formatting.\n";
        assert!(parse_pkl(b"", io, Some(1), &FILES).is_err());
        assert!(parse_pkl(b"", b"Could not format `/s/bad.pkl`: x\n", Some(1), &FILES).is_err());
    }

    #[test]
    fn pkl_cannot_report_a_file_outside_the_run() {
        let err = parse_pkl(b"/elsewhere.pkl\n", b"", Some(11), &FILES).expect_err("fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let stderr = b"Could not format `/elsewhere.pkl`: x at (1:1 - 1:2)\nAn error occurred during formatting.\n";
        let err = parse_pkl(b"", stderr, Some(1), &FILES).expect_err("fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }

    #[test]
    fn pkl_backslash_paths_resolve_only_on_windows() {
        let files = ["m/a.pkl"];
        let listed = parse_pkl(b"m\\a.pkl\nm/a.pkl\n", b"", Some(11), &files);
        let stderr = b"Could not format `m\\a.pkl`: x at (2:3 - 2:4)\nAn error occurred during formatting.\n";
        let located = parse_pkl(b"", stderr, Some(1), &files);
        if cfg!(windows) {
            assert_eq!(listed.expect("ok").len(), 1);
            assert_eq!(located.expect("ok")[0].file, "m/a.pkl");
        } else {
            assert!(matches!(listed, Err(ParseError::UnknownFile { .. })));
            assert!(matches!(located, Err(ParseError::UnknownFile { .. })));
        }
    }
}
