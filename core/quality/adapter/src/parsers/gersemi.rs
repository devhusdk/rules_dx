use super::{
    FileFinding, ParseError, as_text, check_output_size, code_name, finding, known_native,
    unformatted_at,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "gersemi";

const EXIT_FAILED: i32 = 123;

pub fn parse_gersemi(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let diff = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;

    let diffed = diff_files(diff, files)?;
    let mut dirty: Vec<&str> = Vec::new();
    let mut failures: Vec<FileFinding> = Vec::new();

    let lines: Vec<&str> = errors.lines().collect();
    let mut at = 0;
    while let Some(&line) = lines.get(at) {
        at += 1;
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("Warning: unknown command ") {
            while lines.get(at).is_some_and(|next| !next.trim().is_empty()) {
                at += 1;
            }
            continue;
        }
        if let Some(path) = line.strip_suffix(" would be reformatted") {
            let file = known_native(TOOL, files, path)?;
            if !dirty.contains(&file) {
                dirty.push(file);
            }
            continue;
        }
        let Some((file, rest)) = split_known(files, line) else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognized stderr line: {line}"),
            ));
        };
        let (position, message, is_located) = located(rest);
        failures.push(finding(
            TOOL,
            file,
            "syntax".to_owned(),
            message.to_owned(),
            ToolSeverity::Error,
            position,
            ColumnEncoding::Scalars,
        ));
        if is_located {
            if lines.get(at).is_some_and(|next| next.trim() == "^") {
                at += 1;
            } else if lines.get(at + 1).is_some_and(|next| next.trim() == "^") {
                at += 2;
            }
        }
    }

    for (file, _) in &diffed {
        if !dirty.contains(file) {
            return Err(ParseError::shape(
                TOOL,
                format!("{file} has a diff but is not reported as reformatted"),
            ));
        }
    }
    for file in &dirty {
        if !diffed.iter().any(|(name, _)| name == file) {
            return Err(ParseError::shape(
                TOOL,
                format!("{file} is reported as reformatted without a diff"),
            ));
        }
    }

    let want = if !failures.is_empty() {
        Some(EXIT_FAILED)
    } else if dirty.is_empty() {
        Some(0)
    } else {
        Some(1)
    };
    if code != want {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "unexpected exit {} with {} unformatted and {} failed files",
                code_name(code),
                dirty.len(),
                failures.len()
            ),
        ));
    }

    let mut findings = Vec::with_capacity(dirty.len() + failures.len());
    for (file, line) in diffed {
        findings.push(unformatted_at(TOOL, file, line, 1, ColumnEncoding::Scalars));
    }
    findings.extend(failures);
    Ok(findings)
}

fn split_known<'a, 'b>(files: &[&'a str], line: &'b str) -> Option<(&'a str, &'b str)> {
    files.iter().find_map(|file| {
        line.strip_prefix(file)
            .and_then(|rest| rest.strip_prefix(": "))
            .map(|rest| (*file, rest))
    })
}

fn located(rest: &str) -> (TextPosition, &str, bool) {
    let mut parts = rest.splitn(3, ':');
    if let (Some(line), Some(column), Some(message)) = (parts.next(), parts.next(), parts.next()) {
        if let (Ok(line), Ok(_)) = (line.trim().parse::<u64>(), column.trim().parse::<u64>()) {
            return (TextPosition { line, column: 1 }, message.trim(), true);
        }
    }
    (TextPosition { line: 1, column: 1 }, rest.trim(), false)
}

fn diff_files<'a>(diff: &str, files: &[&'a str]) -> Result<Vec<(&'a str, u64)>, ParseError> {
    let mut found: Vec<(&'a str, u64)> = Vec::new();
    let mut lines = diff.lines().peekable();
    while let Some(line) = lines.next() {
        if line.is_empty() {
            continue;
        }
        let Some(old) = line.strip_prefix("--- ") else {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected stdout line: {line}"),
            ));
        };
        let Some(new) = lines.next().and_then(|next| next.strip_prefix("+++ ")) else {
            return Err(ParseError::shape(TOOL, "diff without a +++ header"));
        };
        if old != new {
            return Err(ParseError::shape(
                TOOL,
                format!("diff headers disagree: {old} and {new}"),
            ));
        }
        let file = known_native(TOOL, files, new)?;
        let mut first = None;
        while lines.peek().is_some() {
            if first.is_some() && is_header_pair(&lines) {
                break;
            }
            let next = lines.next().unwrap_or_default();
            if first.is_none()
                && let Some(hunk) = next.strip_prefix("@@ -")
            {
                let start = hunk.split([',', ' ']).next().unwrap_or("");
                first = start.parse::<u64>().ok();
                if first.is_none() {
                    return Err(ParseError::shape(TOOL, format!("bad hunk header: {next}")));
                }
            }
        }
        let Some(first) = first else {
            return Err(ParseError::shape(TOOL, "diff without a hunk"));
        };
        if found.iter().any(|(name, _)| *name == file) {
            return Err(ParseError::shape(TOOL, format!("{file} diffed twice")));
        }
        found.push((file, first.max(1)));
    }
    Ok(found)
}

fn is_header_pair(lines: &std::iter::Peekable<std::str::Lines<'_>>) -> bool {
    let mut ahead = lines.clone();
    let (Some(old), Some(new)) = (ahead.next(), ahead.next()) else {
        return false;
    };
    match (old.strip_prefix("--- "), new.strip_prefix("+++ ")) {
        (Some(old), Some(new)) => old == new,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 4] = [
        "/w/a.cmake",
        "/w/b.cmake",
        "/w/bad.cmake",
        "/w/sp ace/c.cmake",
    ];
    const DIRTY: &str =
        "--- /w/b.cmake\n+++ /w/b.cmake\n@@ -1,3 +1,3 @@\n-project(  foo )\n+project(foo)\n";
    const DIRTY_ERR: &str = "/w/b.cmake would be reformatted\n";
    const BROKEN: &str = "/w/bad.cmake: 3:2: unbalanced parentheses\n\n ^\n\n";

    #[test]
    fn clean_run_has_no_findings() {
        let actual = parse_gersemi(b"", b"", Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
    }

    #[test]
    fn a_diff_reports_the_file_at_the_first_hunk() {
        let findings =
            parse_gersemi(DIRTY.as_bytes(), DIRTY_ERR.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/w/b.cmake");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn a_syntax_error_is_located_and_exits_123() {
        let findings = parse_gersemi(b"", BROKEN.as_bytes(), Some(123), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/w/bad.cmake");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[0].finding.message, "unbalanced parentheses");
    }

    #[test]
    fn the_source_line_after_a_syntax_error_is_skipped() {
        let stderr = "/w/bad.cmake: 5:15: unspecified parsing error\nmessage(\"hi\" \"unterminated\n              ^\n\n";
        let findings = parse_gersemi(b"", stderr.as_bytes(), Some(123), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.column, 1);
    }

    #[test]
    fn a_broken_file_does_not_hide_or_change_the_rest_of_the_batch() {
        let stderr = format!("{BROKEN}{DIRTY_ERR}");
        let findings =
            parse_gersemi(DIRTY.as_bytes(), stderr.as_bytes(), Some(123), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/w/b.cmake");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].file, "/w/bad.cmake");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn a_file_failure_without_a_position_is_still_a_finding() {
        let stderr = "/w/a.cmake: stream did not contain valid UTF-8\n";
        let findings = parse_gersemi(b"", stderr.as_bytes(), Some(123), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 1);
    }

    #[test]
    fn paths_with_spaces_and_removed_dashes_survive() {
        let diff =
            "--- /w/sp ace/c.cmake\n+++ /w/sp ace/c.cmake\n@@ -4 +4 @@\n--- not a header\n+x\n";
        let stderr = "/w/sp ace/c.cmake would be reformatted\n";
        let findings =
            parse_gersemi(diff.as_bytes(), stderr.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 4);
    }

    #[test]
    fn unknown_command_warnings_are_neither_findings_nor_errors() {
        let stderr = "Warning: unknown command 'foo' used at:\n/w/a.cmake:1:1\n\n";
        let actual = parse_gersemi(b"", stderr.as_bytes(), Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
    }

    #[test]
    fn unknown_command_warnings_with_many_call_sites_are_skipped() {
        let stderr = "Warning: unknown command 'foo' used at:\n/w/a.cmake:1:1\n/w/a.cmake:7:1\n/w/b.cmake:2:3\n\nWarning: unknown command 'bar' used at:\n/w/a.cmake:9:1\n\n";
        let actual = parse_gersemi(b"", stderr.as_bytes(), Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
    }

    #[test]
    fn back_to_back_unlocated_failures_are_both_reported() {
        let stderr = "/w/a.cmake: stream did not contain valid UTF-8\n/w/b.cmake: stream did not contain valid UTF-8\n";
        let findings = parse_gersemi(b"", stderr.as_bytes(), Some(123), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[1].file, "/w/b.cmake");
    }

    #[test]
    fn anything_else_is_an_error() {
        let missing = b"[Errno 2] No such file or directory: '/w/.gersemirc'\n";
        assert!(parse_gersemi(b"", missing, Some(1), &FILES).is_err());
        let key = b"/w/.gersemirc: these options are not supported: bogus\n";
        assert!(parse_gersemi(b"", key, Some(0), &FILES).is_err());
        let path = b"Source path doesn't exist: /w/x.cmake\n";
        assert!(parse_gersemi(b"", path, Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"", b"", Some(123), &FILES).is_err());
        assert!(parse_gersemi(b"", b"", None, &FILES).is_err());
        assert!(parse_gersemi(DIRTY.as_bytes(), DIRTY_ERR.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_gersemi(b"", BROKEN.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"", BROKEN.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_gersemi(DIRTY.as_bytes(), b"", Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"", DIRTY_ERR.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"garbage\n", b"", Some(1), &FILES).is_err());
        assert!(parse_gersemi(b"--- /w/b.cmake\n", b"", Some(1), &FILES).is_err());
        let stranger = DIRTY.replace("/w/b.cmake", "/w/other.cmake");
        let stderr = DIRTY_ERR.replace("/w/b.cmake", "/w/other.cmake");
        assert!(parse_gersemi(stranger.as_bytes(), stderr.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_gersemi(&[0xff], b"", Some(0), &FILES).is_err());
        assert!(parse_gersemi(b"", b"Warning: something else\n", Some(0), &FILES).is_err());
    }
}
