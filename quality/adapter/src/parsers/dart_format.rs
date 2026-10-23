use std::path::Path;

use super::{as_text, code_name, finding, known_relative, unformatted_at, FileFinding, ParseError};
use crate::{ColumnEncoding, ToolSeverity};

const TOOL: &str = "dart_format";

/// Exit 0 is clean, 1 names each changed file, and 65 adds located parse errors.
pub fn parse_dart_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0) | Some(1) | Some(65)) {
        return Err(shape(format!("unexpected exit {}", code_name(code))));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in as_text(TOOL, stdout)?.lines() {
        let Some(path) = line.strip_prefix("Changed ") else {
            continue;
        };
        let file = known_relative(TOOL, files, cwd, path.trim_end())?;
        if !findings.iter().any(|found| found.file == file) {
            findings.push(unformatted_at(TOOL, file, 1, 1));
        }
    }
    let changed = findings.len();
    if code == Some(65) {
        for line in as_text(TOOL, stderr)?.lines() {
            let Some((line_no, column, path, message)) = parse_error(line) else {
                continue;
            };
            let file = known_relative(TOOL, files, cwd, path)?;
            let mut found = finding(
                TOOL,
                file,
                String::new(),
                message.to_owned(),
                ToolSeverity::Error,
                line_no,
                column,
            );
            found.finding.encoding = ColumnEncoding::Utf16;
            findings.push(found);
        }
    }
    match code {
        Some(0) if changed > 0 => Err(shape("exit 0 with changed files".to_owned())),
        Some(1) if changed == 0 => Err(shape("exit 1 with no changed files".to_owned())),
        Some(65) if findings.len() == changed => {
            Err(shape("exit 65 with no located parse errors".to_owned()))
        }
        _ => Ok(findings),
    }
}

/// The line, column, path and message of a `line L, column C of <path>: <message>` line.
fn parse_error(line: &str) -> Option<(u64, u64, &str, &str)> {
    let rest = line.strip_prefix("line ")?;
    let (line_no, rest) = rest.split_once(", column ")?;
    let (column, rest) = rest.split_once(" of ")?;
    let (path, message) = rest.split_once(": ")?;
    let message = message.trim();
    if path.is_empty() || message.is_empty() {
        return None;
    }
    Some((line_no.parse().ok()?, column.parse().ok()?, path, message))
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> &'static Path {
        Path::new("/tmp/dx-scratch")
    }

    fn files() -> [&'static str; 2] {
        [
            "/tmp/dx-scratch/app/lib/a.dart",
            "/tmp/dx-scratch/app/lib/b.dart",
        ]
    }

    /// `dart format --output=none --set-exit-if-changed` stdout, verbatim from Dart 3.13.2.
    const DIRTY: &str =
        "Changed /tmp/dx-scratch/app/lib/a.dart\nFormatted 2 files (1 changed) in 0.01 seconds.\n";

    /// The parse error report Dart 3.13.2 writes to stderr, paths spelled from the working directory.
    const BROKEN: &str = "Could not format because the source could not be parsed:\n\nline 2, column 1 of app/lib/b.dart: Expected an identifier.\n  \u{2577}\n2 \u{2502} \n  \u{2502} ^\n  \u{2575}\nline 2, column 1 of app/lib/b.dart: Expected to find '}'.\n  \u{2577}\n2 \u{2502} \n  \u{2502} ^\n  \u{2575}\n";

    #[test]
    fn dart_format_reports_changed_files() {
        let findings =
            parse_dart_format(DIRTY.as_bytes(), b"", Some(1), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/app/lib/a.dart");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let clean = parse_dart_format(
            b"Formatted 2 files (0 changed) in 0.01 seconds.\n",
            b"",
            Some(0),
            &files(),
            scratch(),
        )
        .expect("parsed");
        assert!(clean.is_empty());
    }

    #[test]
    fn dart_format_reports_located_parse_errors() {
        let findings = parse_dart_format(
            DIRTY.as_bytes(),
            BROKEN.as_bytes(),
            Some(65),
            &files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[1].file, "/tmp/dx-scratch/app/lib/b.dart");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].finding.message, "Expected an identifier.");
        assert_eq!(findings[1].finding.start.line, 2);
        assert_eq!(findings[1].finding.start.column, 1);
        assert_eq!(findings[1].finding.encoding, ColumnEncoding::Utf16);
        assert_eq!(findings[2].finding.message, "Expected to find '}'.");
    }

    #[test]
    fn dart_format_rejects_outputs_outside_the_grammar() {
        assert!(parse_dart_format(b"", b"", Some(1), &files(), scratch()).is_err());
        assert!(parse_dart_format(DIRTY.as_bytes(), b"", Some(0), &files(), scratch()).is_err());
        assert!(parse_dart_format(b"", b"", Some(65), &files(), scratch()).is_err());
        assert!(parse_dart_format(b"", b"usage", Some(64), &files(), scratch()).is_err());
        assert!(parse_dart_format(b"", b"", None, &files(), scratch()).is_err());
        assert!(parse_dart_format(&[0xff], b"", Some(0), &files(), scratch()).is_err());
    }

    #[test]
    fn dart_format_cannot_report_a_file_outside_the_run() {
        let err = parse_dart_format(
            b"Changed /tmp/dx-scratch/app/lib/c.dart\n",
            b"",
            Some(1),
            &files(),
            scratch(),
        )
        .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_dart_format(
            b"",
            b"line 1, column 1 of ../elsewhere/b.dart: Expected an identifier.\n",
            Some(65),
            &files(),
            scratch(),
        )
        .expect_err("climbing out of the run fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }
}
