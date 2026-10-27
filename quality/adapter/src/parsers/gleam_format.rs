use crate::{ColumnEncoding, TextPosition};
use std::path::Path;

use super::{
    as_text, code_name, finding, known_relative, path_row_col, unformatted_at, FileFinding,
    ParseError,
};
use crate::ToolSeverity;

const TOOL: &str = "gleam_format";

const UNFORMATTED_HEADER: &str = "error: These files have not been formatted";
const SYNTAX_HEADER: &str = "error: Syntax error";
const LISTED_PREFIX: &str = "- ";
const LOCATION_PREFIX: &str = "┌─ ";

/// Findings from `gleam format --check`: listed unformatted files, or the one file it could not parse.
pub fn parse_gleam_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    if !as_text(TOOL, stdout)?.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "--check wrote to stdout"));
    }
    let raw = as_text(TOOL, stderr)?;
    let lines: Vec<&str> = raw.lines().collect();
    let header = lines.iter().position(|line| !line.trim().is_empty());
    let findings = match header.map(|at| (at, lines[at].trim_end())) {
        Some((at, UNFORMATTED_HEADER)) => unformatted(files, cwd, &lines[at + 1..])?,
        Some((at, SYNTAX_HEADER)) => vec![syntax_error(files, cwd, &lines[at + 1..])?],
        _ => Vec::new(),
    };
    let shaped = match code {
        Some(0) => raw.trim().is_empty(),
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

fn unformatted(files: &[&str], cwd: &Path, rest: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in rest {
        let Some(path) = line.trim().strip_prefix(LISTED_PREFIX) else {
            continue;
        };
        let file = known_relative(TOOL, files, cwd, path.trim())?;
        if !findings.iter().any(|found| found.file == file) {
            findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
        }
    }
    Ok(findings)
}

fn syntax_error(files: &[&str], cwd: &Path, rest: &[&str]) -> Result<FileFinding, ParseError> {
    let location = rest
        .iter()
        .find_map(|line| line.trim().strip_prefix(LOCATION_PREFIX))
        .ok_or_else(|| ParseError::shape(TOOL, "syntax error without a location"))?;
    let (path, line, column) = path_row_col(location.trim())
        .ok_or_else(|| ParseError::shape(TOOL, format!("unreadable location {location}")))?;
    let file = known_relative(TOOL, files, cwd, path)?;
    Ok(finding(
        TOOL,
        file,
        "syntax".to_owned(),
        message(rest),
        ToolSeverity::Error,
        TextPosition { line, column },
        ColumnEncoding::Scalars,
    ))
}

/// The caret label, followed by the `Found ...` explanation when Gleam gives one.
fn message(rest: &[&str]) -> String {
    let caret = rest.iter().position(|line| {
        line.trim_start()
            .strip_prefix('│')
            .is_some_and(|tail| tail.trim_start().starts_with('^'))
    });
    let Some(at) = caret else {
        return "could not parse the file".to_owned();
    };
    let label = rest[at]
        .trim_start()
        .trim_start_matches('│')
        .trim_start()
        .trim_start_matches('^')
        .trim();
    let label = if label.is_empty() {
        "could not parse the file"
    } else {
        label
    };
    let detail: Vec<&str> = rest[at + 1..]
        .iter()
        .map(|line| line.trim())
        .skip_while(|line| line.is_empty())
        .take_while(|line| !line.is_empty())
        .collect();
    match detail.split_first() {
        Some((first, items)) if first.starts_with("Found ") => {
            let items: Vec<&str> = items
                .iter()
                .map(|item| item.trim_start_matches(LISTED_PREFIX).trim())
                .collect();
            let found = first.trim_end_matches(':').trim_end();
            if items.is_empty() {
                format!("{label}: {found}")
            } else {
                format!("{label}: {found}: {}", items.join(", "))
            }
        }
        _ => label.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const CWD: &str = "/s";
    const DIRTY: &str = "error: These files have not been formatted\n\n  - /s/src/a.gleam\n  - /s/src/d.gleam\n\n\n";
    const UNEXPECTED: &str = "error: Syntax error\n  \u{250c}\u{2500} /s/src/c.gleam:1:13\n  \u{2502}\n1 \u{2502} pub fn bad( {\n  \u{2502}             ^ I was not expecting this\n\nFound `{`, expected one of: \n- `)`\n- a function parameter\n\n";
    const OPERATOR: &str = "error: Syntax error\n  \u{250c}\u{2500} src/e.gleam:2:5\n  \u{2502}\n2 \u{2502}   1 +\n  \u{2502}     ^ This operator has no value on its right side\n\nHint: Remove it or put a value after it.\n";
    const IO_FAILURE: &str = "error: File IO failure\n\nAn error occurred while trying to read this file:\n\n    /s/src/missing.gleam\n\nThe error message from the file IO library was:\n\n    No such file or directory (os error 2)\n";

    fn cwd() -> &'static Path {
        Path::new(CWD)
    }

    #[test]
    fn gleam_format_reports_each_listed_file_once() {
        let files = ["/s/src/a.gleam", "/s/src/b.gleam", "/s/src/d.gleam"];
        let findings =
            parse_gleam_format(b"", DIRTY.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        let named: Vec<&str> = findings.iter().map(|found| found.file.as_str()).collect();
        assert_eq!(named, ["/s/src/a.gleam", "/s/src/d.gleam"]);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.tool_id, "gleam_format");
        let twice = DIRTY.replace("/s/src/d.gleam", "/s/src/a.gleam");
        assert_eq!(
            parse_gleam_format(b"", twice.as_bytes(), Some(1), &files, cwd())
                .expect("parsed")
                .len(),
            1
        );
        let relative = DIRTY.replace("/s/src/a.gleam", "src/a.gleam");
        let findings =
            parse_gleam_format(b"", relative.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].file, "/s/src/a.gleam");
    }

    #[test]
    fn gleam_format_locates_a_syntax_error_with_its_explanation() {
        let files = ["/s/src/c.gleam"];
        let findings =
            parse_gleam_format(b"", UNEXPECTED.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings.len(), 1);
        let error = &findings[0].finding;
        assert_eq!(findings[0].file, "/s/src/c.gleam");
        assert_eq!(error.rule_id, "syntax");
        assert_eq!(error.severity, ToolSeverity::Error);
        assert_eq!(
            error.message,
            "I was not expecting this: Found `{`, expected one of: `)`, a function parameter"
        );
        assert_eq!(
            error.start,
            TextPosition {
                line: 1,
                column: 13
            }
        );
    }

    #[test]
    fn gleam_format_keeps_the_caret_label_without_a_found_line() {
        let files = ["/s/src/e.gleam"];
        let findings =
            parse_gleam_format(b"", OPERATOR.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].file, "/s/src/e.gleam");
        assert_eq!(
            findings[0].finding.message,
            "This operator has no value on its right side"
        );
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 2, column: 5 }
        );
        let bare = "error: Syntax error\n  \u{250c}\u{2500} src/e.gleam:2:5\n";
        let findings =
            parse_gleam_format(b"", bare.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].finding.message, "could not parse the file");
        let found_only = "error: Syntax error\n  \u{250c}\u{2500} src/e.gleam:2:5\n  \u{2502} ^ Unexpected\n\nFound a name\n";
        let findings =
            parse_gleam_format(b"", found_only.as_bytes(), Some(1), &files, cwd()).expect("parsed");
        assert_eq!(findings[0].finding.message, "Unexpected: Found a name");
    }

    #[test]
    fn gleam_format_rejects_unshaped_output() {
        let files = ["/s/src/a.gleam"];
        assert!(parse_gleam_format(b"", b"", Some(0), &files, cwd())
            .expect("parsed")
            .is_empty());
        assert!(parse_gleam_format(b"", DIRTY.as_bytes(), Some(0), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", b"", Some(1), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", b"noise\n", Some(0), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", IO_FAILURE.as_bytes(), Some(1), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", b"", None, &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", b"", Some(2), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"x\n", b"", Some(0), &files, cwd()).is_err());
        assert!(parse_gleam_format(b"", &[0xff], Some(1), &files, cwd()).is_err());
        assert!(parse_gleam_format(
            b"",
            "error: Syntax error\n  \u{2502}\n".as_bytes(),
            Some(1),
            &files,
            cwd()
        )
        .is_err());
        assert!(parse_gleam_format(
            b"",
            "error: Syntax error\n  \u{250c}\u{2500} /s/src/a.gleam:x:1\n".as_bytes(),
            Some(1),
            &files,
            cwd()
        )
        .is_err());
        assert!(matches!(
            parse_gleam_format(b"", DIRTY.as_bytes(), Some(1), &files, cwd()),
            Err(ParseError::UnknownFile { path, .. }) if path == "/s/src/d.gleam"
        ));
        assert!(matches!(
            parse_gleam_format(b"", UNEXPECTED.as_bytes(), Some(1), &files, cwd()),
            Err(ParseError::UnknownFile { path, .. }) if path == "/s/src/c.gleam"
        ));
    }
}
