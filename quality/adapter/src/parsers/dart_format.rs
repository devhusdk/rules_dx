use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, finding, known, unformatted_at, FileFinding, ParseError};

const TOOL: &str = "dart";

const CHANGED: &str = "Changed ";
const PARSE_FAILURE: &str = "Could not format because the source could not be parsed:";

/// Findings from `dart format --output=none --set-exit-if-changed`.
///
/// stdout lists each unformatted file as `Changed <path>` and ends in a `Formatted N files
/// (M changed)` summary; stderr holds one `line L, column C of <path>: <message>` entry per syntax
/// error. The exit code must agree with the output: 0 is clean, 1 lists changed files, and 65 is a
/// parse failure with a located error. The summary must account for every checked file, so a file
/// the tool never reached cannot pass.
pub fn parse_dart_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    let mut changed: Vec<&str> = Vec::new();
    let mut summary = None;
    for line in out.lines() {
        if let Some(path) = line.strip_prefix(CHANGED) {
            let path = known(TOOL, files, path)?;
            if !changed.contains(&path) {
                changed.push(path);
            }
        } else if let Some(counts) = summary_counts(line) {
            if summary.replace(counts).is_some() {
                return Err(shape("more than one summary line"));
            }
        }
    }
    let Some((formatted, reported)) = summary else {
        return Err(shape(&format!(
            "exit {} without a summary line",
            code_name(code)
        )));
    };
    if reported != changed.len() {
        return Err(shape(&format!(
            "summary counts {reported} changed files but {} were listed",
            changed.len()
        )));
    }
    let errors = syntax_errors(err, files)?;
    let mut failed: Vec<&str> = Vec::new();
    for error in &errors {
        if !failed.contains(&error.file.as_str()) {
            failed.push(error.file.as_str());
        }
    }
    match code {
        Some(0) if changed.is_empty() && errors.is_empty() => {}
        Some(1) if !changed.is_empty() && errors.is_empty() => {}
        Some(65) if !errors.is_empty() => {}
        _ => {
            return Err(shape(&format!(
                "exit {} with {} changed files and {} syntax errors",
                code_name(code),
                changed.len(),
                errors.len()
            )));
        }
    }
    if formatted + failed.len() != files.len() {
        return Err(shape(&format!(
            "{formatted} files formatted and {} unparsable of {} checked",
            failed.len(),
            files.len()
        )));
    }
    let mut findings: Vec<FileFinding> = changed
        .into_iter()
        .map(|file| unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars))
        .collect();
    findings.extend(errors);
    Ok(findings)
}

fn shape(detail: &str) -> ParseError {
    ParseError::shape(TOOL, detail.to_owned())
}

/// `(files formatted, files changed)` from `Formatted N file(s) (M changed) in ...`.
fn summary_counts(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("Formatted ")?;
    if rest.starts_with("no files") {
        return Some((0, 0));
    }
    let (count, rest) = rest.split_once(' ')?;
    let rest = rest
        .strip_prefix("files (")
        .or_else(|| rest.strip_prefix("file ("))?;
    let (changed, _) = rest.split_once(" changed)")?;
    Some((count.parse().ok()?, changed.parse().ok()?))
}

fn syntax_errors(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut out = Vec::new();
    for line in stderr.lines() {
        let Some(rest) = line.strip_prefix("line ") else {
            continue;
        };
        let Some((position, tail)) = rest.split_once(" of ") else {
            continue;
        };
        let Some((row, column)) = position.split_once(", column ") else {
            continue;
        };
        let (Ok(row), Ok(column)) = (row.parse::<u64>(), column.parse::<u64>()) else {
            continue;
        };
        let Some((path, message)) = split_path(tail, files) else {
            return Err(ParseError::UnknownFile {
                tool: TOOL,
                path: tail.split(": ").next().unwrap_or(tail).to_owned(),
            });
        };
        out.push(finding(
            TOOL,
            path,
            "syntax".to_owned(),
            message.trim().to_owned(),
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    if out.is_empty() && stderr.contains(PARSE_FAILURE) {
        return Err(shape("parse failure without a located error"));
    }
    Ok(out)
}

/// The longest checked file the text starts with, followed by `: `.
/// Splits `<path>: <message>`. Dart spells a path relative to its working directory, climbing
/// with `../` when the file sits outside it, so a spelling also matches a checked file that ends
/// with the spelled path once the leading `../` components are dropped.
fn split_path<'a>(text: &'a str, files: &[&'a str]) -> Option<(&'a str, &'a str)> {
    let (spelled, message) = text.split_once(": ")?;
    let tail = {
        let mut rest = spelled;
        while let Some(next) = rest.strip_prefix("../") {
            rest = next;
        }
        rest
    };
    let exact = files.iter().copied().find(|file| *file == spelled);
    let found = exact.or_else(|| {
        let mut hits = files.iter().copied().filter(|file| {
            !tail.is_empty()
                && (*file == tail
                    || file
                        .strip_suffix(tail)
                        .is_some_and(|head| head.ends_with('/')))
        });
        let first = hits.next()?;
        hits.next().is_none().then_some(first)
    });
    found.map(|file| (file, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/s/dirty.dart", "/s/clean.dart", "/s/bad.dart"];
    const DIRTY: &str = "Changed /s/dirty.dart\nFormatted 2 files (1 changed) in 0.01 seconds.\n";
    const CLEAN: &str = "Formatted 3 files (0 changed) in 0.01 seconds.\n";
    const BROKEN: &str = "Could not format because the source could not be parsed:\n\nline 2, column 1 of /s/bad.dart: Expected an identifier.\n  ╷\n2 │ \n  │ ^\n  ╵\n";
    const PARSE_SUMMARY: &str = "Formatted 2 files (0 changed) in 0.00 seconds.\n";

    #[test]
    fn dart_format_reports_changed_files() {
        let files = &FILES[..2];
        let findings = parse_dart_format(DIRTY.as_bytes(), b"", Some(1), files).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.dart");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert!(parse_dart_format(
            b"Formatted 2 files (0 changed) in 0 seconds.\n",
            b"",
            Some(0),
            files
        )
        .expect("parsed")
        .is_empty());
    }

    #[test]
    fn dart_format_fails_closed_when_exit_and_output_disagree() {
        let files = &FILES[..2];
        assert!(parse_dart_format(DIRTY.as_bytes(), b"", Some(0), files).is_err());
        assert!(parse_dart_format(
            b"Formatted 2 files (0 changed) in 0 seconds.\n",
            b"",
            Some(1),
            files
        )
        .is_err());
        assert!(parse_dart_format(b"", b"", Some(0), files).is_err());
        assert!(parse_dart_format(b"", b"", Some(1), files).is_err());
        assert!(parse_dart_format(DIRTY.as_bytes(), b"", Some(2), files).is_err());
        assert!(parse_dart_format(DIRTY.as_bytes(), b"", None, files).is_err());
        assert!(parse_dart_format(
            b"Changed /s/other.dart\nFormatted 2 files (1 changed) in 0 seconds.\n",
            b"",
            Some(1),
            files
        )
        .is_err());
        assert!(parse_dart_format(
            b"Changed /s/dirty.dart\nFormatted 2 files (2 changed) in 0 seconds.\n",
            b"",
            Some(1),
            files
        )
        .is_err());
        assert!(parse_dart_format(&[0xff], b"", Some(0), files).is_err());
        assert!(parse_dart_format(CLEAN.as_bytes(), &[0xff], Some(0), files).is_err());
    }

    #[test]
    fn dart_format_matches_paths_spelled_from_its_working_directory() {
        let files = ["/work/scratch/matrix/bad.dart"];
        let stderr = "Could not format because the source could not be parsed:\n\nline 2, column 1 of ../work/scratch/matrix/bad.dart: Expected an identifier.\n";
        let findings = parse_dart_format(
            b"Formatted no files in 0.00 seconds.\n",
            stderr.as_bytes(),
            Some(65),
            &files,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, files[0]);
    }

    #[test]
    fn dart_format_requires_every_file_to_be_accounted_for() {
        assert!(parse_dart_format(
            b"Formatted no files in 0.00 seconds.\n",
            b"",
            Some(0),
            &FILES[..2]
        )
        .is_err());
        assert!(parse_dart_format(
            b"Formatted 1 file (0 changed) in 0.00 seconds.\n",
            b"",
            Some(0),
            &FILES[..2]
        )
        .is_err());
        assert!(parse_dart_format(CLEAN.as_bytes(), b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn dart_format_reports_syntax_errors_at_their_location() {
        let files = &FILES[1..];
        let overcounted =
            parse_dart_format(PARSE_SUMMARY.as_bytes(), BROKEN.as_bytes(), Some(65), files);
        assert!(overcounted.is_err(), "an unreached file must not pass");
        let findings = parse_dart_format(
            b"Formatted 1 file (0 changed) in 0.00 seconds.\n",
            BROKEN.as_bytes(),
            Some(65),
            files,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        let error = &findings[0];
        assert_eq!(error.file, "/s/bad.dart");
        assert_eq!(error.finding.severity, ToolSeverity::Error);
        assert_eq!(error.finding.rule_id, "syntax");
        assert_eq!(error.finding.message, "Expected an identifier.");
        assert_eq!(error.finding.start, TextPosition { line: 2, column: 1 });
    }

    #[test]
    fn dart_format_never_passes_a_parse_failure_without_a_location() {
        let summary = b"Formatted no files in 0.00 seconds.\n";
        let files = ["/s/bad.dart"];
        assert!(parse_dart_format(summary, b"", Some(65), &files).is_err());
        assert!(parse_dart_format(summary, PARSE_FAILURE.as_bytes(), Some(65), &files).is_err());
        assert!(parse_dart_format(summary, BROKEN.as_bytes(), Some(0), &files).is_err());
        assert!(parse_dart_format(
            summary,
            b"line 1, column 1 of /s/other.dart: nope\n",
            Some(65),
            &files
        )
        .is_err());
    }
}
