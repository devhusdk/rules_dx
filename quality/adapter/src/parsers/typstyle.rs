use std::path::Path;

use super::{
    as_text, code_name, finding, known_relative, unformatted_at, FileFinding, ParseError,
};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

const TOOL: &str = "typstyle";

const WOULD_REFORMAT: &str = "Would reformat: ";
const FAILED_TO_PARSE: &str = "warn: Failed to parse ";
const ERRONEOUS: &str = ". The source is erroneous.";

/// `--check` prints `Would reformat: <path>` on stdout for each file it would rewrite and exits 1.
/// A file with a syntax error is reported on stderr as `warn: Failed to parse <path>. The source
/// is erroneous.` and the exit code stays 0 (or 1 beside a dirty file), in check and in `-i` mode.
/// Paths are spelled from the working directory. Exit 2 is an execution error.
pub fn parse_typstyle(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    let listed = as_text(TOOL, stdout)?;
    let diagnostics = as_text(TOOL, stderr)?;
    if listed.contains('\u{1b}') || diagnostics.contains('\u{1b}') {
        return Err(ParseError::shape(TOOL, "unexpected ANSI escape in output"));
    }
    match code {
        Some(0 | 1) => {}
        Some(2) => {
            let detail = diagnostics
                .lines()
                .find(|line| line.starts_with("error:"))
                .unwrap_or("exit 2")
                .to_owned();
            return Err(ParseError::Incomplete {
                tool: TOOL,
                detail,
            });
        }
        _ => {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected exit {} with output", code_name(code)),
            ))
        }
    }

    let mut findings: Vec<FileFinding> = Vec::new();
    for line in listed.lines().map(str::trim_end) {
        if line.is_empty() {
            continue;
        }
        let Some(path) = line.strip_prefix(WOULD_REFORMAT) else {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected stdout line: {line}"),
            ));
        };
        let file = known(files, cwd, path)?;
        if !findings.iter().any(|found| found.file == file) {
            findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
        }
    }
    if code == Some(1) && findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 1 with no listed files",
        ));
    }
    if code == Some(0) && !findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 0 with files that would be reformatted",
        ));
    }

    for line in diagnostics.lines().map(str::trim_end) {
        if line.is_empty() {
            continue;
        }
        if line.starts_with("error:") {
            return Err(ParseError::Incomplete {
                tool: TOOL,
                detail: line.to_owned(),
            });
        }
        let path = line
            .strip_prefix(FAILED_TO_PARSE)
            .and_then(|rest| rest.strip_suffix(ERRONEOUS));
        let Some(path) = path else {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected stderr line: {line}"),
            ));
        };
        let file = known(files, cwd, path)?;
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            "syntax error: the source is erroneous".to_owned(),
            ToolSeverity::Error,
            TextPosition { line: 1, column: 1 },
            ColumnEncoding::Scalars,
        ));
    }
    Ok(findings)
}

/// The checked file a path names, accepting a path spelled from the working directory with
/// either separator.
fn known<'a>(files: &[&'a str], cwd: &Path, path: &str) -> Result<&'a str, ParseError> {
    match known_relative(TOOL, files, cwd, path) {
        Err(_) if path.contains('\\') => known_relative(TOOL, files, cwd, &path.replace('\\', "/")),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/typst/a.typ", "/s/typst/lib/b.typ"];

    fn parse(stdout: &str, stderr: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_typstyle(
            stdout.as_bytes(),
            stderr.as_bytes(),
            Some(code),
            &FILES,
            Path::new("/s"),
        )
    }

    #[test]
    fn typstyle_clean_is_empty() {
        assert!(parse("", "", 0).expect("parsed").is_empty());
    }

    #[test]
    fn typstyle_lists_files_it_would_reformat() {
        let findings = parse(
            "Would reformat: typst/a.typ\nWould reformat: typst/lib/b.typ\n",
            "",
            1,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/typst/a.typ");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[1].file, "/s/typst/lib/b.typ");
    }

    #[test]
    fn typstyle_accepts_absolute_and_windows_spellings() {
        let findings = parse("Would reformat: /s/typst/a.typ\n", "", 1).expect("parsed");
        assert_eq!(findings[0].file, "/s/typst/a.typ");
        let findings = parse("Would reformat: typst\\lib\\b.typ\n", "", 1).expect("parsed");
        assert_eq!(findings[0].file, "/s/typst/lib/b.typ");
    }

    #[test]
    fn typstyle_exit_one_without_files_is_malformed() {
        assert!(parse("", "", 1).is_err());
    }

    #[test]
    fn typstyle_syntax_error_at_exit_zero_is_never_clean() {
        let findings = parse(
            "",
            "warn: Failed to parse typst/a.typ. The source is erroneous.\n",
            0,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/typst/a.typ");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(findings[0].finding.message.contains("syntax error"));
    }

    #[test]
    fn typstyle_keeps_dirty_files_beside_a_syntax_error() {
        let findings = parse(
            "Would reformat: typst/a.typ\n",
            "warn: Failed to parse typst/lib/b.typ. The source is erroneous.\n",
            1,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[1].file, "/s/typst/lib/b.typ");
    }

    #[test]
    fn typstyle_exit_two_is_an_execution_error() {
        let error = parse(
            "",
            "error: failed to read x.typ\n  Cause: failed to format 1 file due to IO error\n",
            2,
        )
        .expect_err("execution error");
        assert!(matches!(error, ParseError::Incomplete { .. }));
        assert!(parse("", "", 2).is_err());
    }

    #[test]
    fn typstyle_error_line_at_exit_zero_is_an_error() {
        assert!(parse("", "error: boom\n", 0).is_err());
    }

    #[test]
    fn typstyle_rejects_unknown_exit_junk_and_ansi() {
        assert!(parse("", "", 3).is_err());
        assert!(parse_typstyle(b"", b"", None, &FILES, Path::new("/s")).is_err());
        assert!(parse("junk\n", "", 0).is_err());
        assert!(parse("", "something odd\n", 0).is_err());
        assert!(parse("\u{1b}[1mWould reformat: typst/a.typ\u{1b}[0m\n", "", 1).is_err());
        assert!(parse("Would reformat: typst/a.typ\n", "", 0).is_err());
    }

    #[test]
    fn typstyle_rejects_a_path_outside_the_checked_files() {
        let error = parse("Would reformat: other.typ\n", "", 1).expect_err("unknown");
        assert!(matches!(error, ParseError::UnknownFile { .. }));
        assert!(parse(
            "",
            "warn: Failed to parse other.typ. The source is erroneous.\n",
            0
        )
        .is_err());
    }
}
