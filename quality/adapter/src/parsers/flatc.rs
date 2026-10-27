use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "flatc";

/// What one `flatc -M --cpp` run reported about one schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatcReport {
    /// Errors and warnings flatc printed on stderr.
    pub findings: Vec<FileFinding>,
    /// Every file the make rule on stdout lists as a dependency, in order.
    pub dependencies: Vec<String>,
}

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

fn severity(text: &str) -> Option<ToolSeverity> {
    match text {
        "error" => Some(ToolSeverity::Error),
        "warning" => Some(ToolSeverity::Warning),
        _ => None,
    }
}

/// Reads `path:line: col: error|warning: message` (gcc style) or
/// `path(line, col): error|warning: message` (the `_WIN32` style).
fn locate(line: &str) -> Option<Located<'_>> {
    for (at, _) in line.match_indices(": ") {
        let (head, tail) = (&line[..at], &line[at + 2..]);
        let (kind, message) = tail.split_once(": ")?;
        let Some(severity) = severity(kind) else {
            continue;
        };
        if let Some(open) = head.rfind('(') {
            if let Some(inner) = head[open + 1..].strip_suffix(')') {
                if let Some((row, column)) = inner.split_once(", ") {
                    if let (Some(row), Some(column)) = (number(row), number(column)) {
                        return Some(Located {
                            path: &line[..open],
                            line: row,
                            column,
                            severity,
                            message,
                        });
                    }
                }
            }
        }
        let (rest, column) = head.rsplit_once(": ").unwrap_or((head, ""));
        if let (Some((path, row)), Some(column)) = (rest.rsplit_once(':'), number(column)) {
            if let Some(row) = number(row) {
                return Some(Located {
                    path,
                    line: row,
                    column,
                    severity,
                    message,
                });
            }
        }
    }
    None
}

/// Splits the make rule `target: dep dep ...` that `-M` prints, with the
/// ` \` line continuations joined, into the dependency list.
fn dependencies(stdout: &str) -> Result<Vec<String>, ParseError> {
    let joined = stdout.replace(" \\\r\n", " ").replace(" \\\n", " ");
    let mut deps = Vec::new();
    for rule in joined.lines().filter(|line| !line.trim().is_empty()) {
        let Some((_, list)) = rule.split_once(": ") else {
            return Err(ParseError::shape(
                TOOL,
                format!("make rule without a target: {rule}"),
            ));
        };
        for dep in list.split_whitespace() {
            let absolute = dep.starts_with('/')
                || dep.starts_with("\\\\")
                || (dep.len() > 2 && dep.as_bytes()[1] == b':');
            if !absolute {
                return Err(ParseError::shape(
                    TOOL,
                    format!("dependency is not an absolute path: {dep}"),
                ));
            }
            deps.push(dep.to_owned());
        }
    }
    Ok(deps)
}

/// Parses the stderr and the `-M` make rule of one flatc run over one schema.
///
/// Warnings are findings. Output the grammar cannot read is never clean: a
/// nonzero exit needs an error finding, a zero exit allows warnings only, and
/// any other stderr line is a shape error.
pub fn parse_flatc(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<FlatcReport, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let out =
        std::str::from_utf8(stdout).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let err =
        std::str::from_utf8(stderr).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let mut findings = Vec::new();
    let mut broken = Vec::new();
    let mut errors = 0usize;
    for line in err.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "error:" || trimmed == "warning:" {
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
            broken.push(format!("{}:{}: {}", found.path, found.line, found.message));
            continue;
        };
        let rule = if found.severity == ToolSeverity::Error {
            "error"
        } else {
            "warning"
        };
        findings.push(FileFinding {
            file: (*file).to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule.to_owned(),
                message: found.message.to_owned(),
                severity: found.severity,
                start: TextPosition {
                    line: found.line,
                    // flatc reports the cursor offset, which is 0 on an empty file.
                    column: found.column.max(1),
                },
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Utf8,
                related: Vec::new(),
            },
        });
    }
    match code {
        Some(0) if errors > 0 => {
            return Err(ParseError::shape(
                TOOL,
                format!("{errors} error(s) with exit 0"),
            ));
        }
        Some(0) => {}
        Some(1) if errors == 0 => {
            return Err(ParseError::shape(
                TOOL,
                "exit 1 with no error finding".to_owned(),
            ));
        }
        Some(1) => {}
        other => {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected exit {}", code_name(other)),
            ));
        }
    }
    if !broken.is_empty() {
        broken.sort();
        broken.dedup();
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("finding outside the checked files: {}", broken.join("; ")),
        });
    }
    let dependencies = if code == Some(0) {
        dependencies(out)?
    } else {
        Vec::new()
    };
    Ok(FlatcReport {
        findings,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/a.fbs";

    #[test]
    fn clean_run_lists_dependencies_with_continuations_joined() {
        let out = b"/s/out/a_generated.h: \\\n /s/b.fbs \\\n /s/a.fbs\n";
        let report = parse_flatc(out, b"", Some(0), &[FILE]).expect("clean");
        assert!(report.findings.is_empty());
        assert_eq!(report.dependencies, ["/s/b.fbs", "/s/a.fbs"]);
    }

    #[test]
    fn error_is_a_finding() {
        let err = b"\nerror:\n  /s/a.fbs:1: 19: error: type referenced but not defined: Nope\n\n";
        let report = parse_flatc(b"", err, Some(1), &[FILE]).expect("findings");
        assert_eq!(report.findings.len(), 1);
        let finding = &report.findings[0];
        assert_eq!(finding.file, FILE);
        assert_eq!(finding.finding.severity, ToolSeverity::Error);
        assert_eq!(
            finding.finding.start,
            TextPosition {
                line: 1,
                column: 19
            }
        );
        assert!(report.dependencies.is_empty());
    }

    #[test]
    fn warning_with_exit_zero_is_a_finding() {
        let out = b"/s/o/a_generated.h: \\\n /s/a.fbs\n";
        let err = b"\nwarning:\n  /s/a.fbs:2: 5: warning: field names should be lowercase\n\n";
        let report = parse_flatc(out, err, Some(0), &[FILE]).expect("warning");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(report.dependencies, ["/s/a.fbs"]);
    }

    #[test]
    fn windows_style_location_is_read() {
        let err = b"\nerror:\n  C:\\s\\a.fbs(3, 7): error: expecting: ;\n";
        let report = parse_flatc(b"", err, Some(1), &["C:\\s\\a.fbs"]).expect("windows");
        assert_eq!(
            report.findings[0].finding.start,
            TextPosition { line: 3, column: 7 }
        );
    }

    #[test]
    fn empty_file_error_has_column_one() {
        let err = b"error:\n  /s/a.fbs:1: 0: error: input file is empty\n";
        let report = parse_flatc(b"", err, Some(1), &[FILE]).expect("empty");
        assert_eq!(report.findings[0].finding.start.column, 1);
    }

    #[test]
    fn unreadable_output_is_never_clean() {
        let missing = b"\nerror:\n  unable to load file: nofile.fbs\n\n";
        assert!(parse_flatc(b"", missing, Some(1), &[FILE]).is_err());
        assert!(parse_flatc(b"", b"", Some(1), &[FILE]).is_err());
        assert!(parse_flatc(b"", b"", None, &[FILE]).is_err());
        assert!(parse_flatc(b"", b"", Some(2), &[FILE]).is_err());
        assert!(parse_flatc(b"", b"garbage\n", Some(0), &[FILE]).is_err());
        let err = b"  /s/a.fbs:1: 2: error: m\n";
        assert!(parse_flatc(b"", err, Some(0), &[FILE]).is_err());
        assert!(parse_flatc(b"a: \\\n b.fbs\n", b"", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn finding_in_an_unchecked_file_is_incomplete() {
        let err = b"error:\n  /s/other.fbs:1: 2: error: m\n";
        assert!(matches!(
            parse_flatc(b"", err, Some(1), &[FILE]),
            Err(ParseError::Incomplete { .. })
        ));
    }
}
