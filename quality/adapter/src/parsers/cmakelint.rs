use super::{as_text, code_name, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "cmakelint";

/// The summary cmakelint writes to stderr unless `quiet` is set.
const TOTAL: &str = "Total Errors: ";

/// Splits one finding line `path:line: message [category]` against the run's own file set.
///
/// The path is matched against the known `files` (longest first) rather than split on the
/// first `:`, so a path that itself holds a colon cannot be misread.
fn split_line<'a>(line: &'a str, files: &[&'a str]) -> Option<(&'a str, u64, &'a str, &'a str)> {
    let mut best: Option<(&str, u64, &str, &str)> = None;
    for file in files {
        let Some(rest) = line.strip_prefix(file).and_then(|r| r.strip_prefix(':')) else {
            continue;
        };
        let Some((number, tail)) = rest.split_once(": ") else {
            continue;
        };
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(number) = number.parse::<u64>() else {
            continue;
        };
        let Some(body) = tail.trim_end().strip_suffix(']') else {
            continue;
        };
        let Some((message, category)) = body.rsplit_once(" [") else {
            continue;
        };
        if message.trim().is_empty() || category.is_empty() || category.contains(['[', ']', ' ']) {
            continue;
        }
        if best.is_none_or(|(name, ..)| name.len() < file.len()) {
            best = Some((file, number, message.trim(), category));
        }
    }
    best
}

/// Parses `cmakelint --config=<config> -- <files>`.
///
/// Findings are line-only: cmakelint reports no column, so each finding is anchored at column
/// 1 of its line (line 0 file-level findings clamp to line 1), counted in Unicode scalars as
/// for checkmake (#3931).
///
/// cmakelint exits 0 quietly in several broken situations (a file it does not recognise
/// prints `Ignoring file:` and is skipped, an unreadable `--config` is ignored), so this never
/// decides from the exit code alone. The run only counts when stderr carries exactly one
/// `Total Errors: N` summary, N equals the number of parsed findings, the exit code agrees
/// with that count, and every stdout line is a finding for one of the known files.
pub fn parse_cmakelint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), stderr.trim()),
        ));
    }
    if stderr.contains("Traceback (most recent call last)") || stderr.contains("FATAL ERROR") {
        return Err(ParseError::shape(
            TOOL,
            format!("cmakelint crashed: {}", stderr.trim()),
        ));
    }
    let text = as_text(TOOL, stdout)?;
    let mut findings = Vec::new();
    for line in text.lines() {
        if line.starts_with("Ignoring file: ") {
            return Err(ParseError::shape(
                TOOL,
                format!("cmakelint skipped a file it does not recognise: {line}"),
            ));
        }
        let Some((file, number, message, category)) = split_line(line, files) else {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "unrecognised output line: {:?}",
                    line.chars().take(120).collect::<String>()
                ),
            ));
        };
        let (start, end) = point(number.max(1), 1);
        findings.push(FileFinding {
            file: file.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: category.to_owned(),
                message: message.to_owned(),
                severity: ToolSeverity::Error,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    let totals: Vec<&str> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix(TOTAL))
        .collect();
    let [total] = totals.as_slice() else {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "stderr must carry exactly one {TOTAL}N summary (stderr: {})",
                stderr.trim().chars().take(200).collect::<String>()
            ),
        ));
    };
    let total: usize = total
        .trim()
        .parse()
        .map_err(|_| ParseError::shape(TOOL, format!("malformed {TOTAL}summary: {total:?}")))?;
    if total != findings.len() {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "{TOTAL}{total} does not match the {} parsed findings",
                findings.len()
            ),
        ));
    }
    match (code, findings.is_empty()) {
        (Some(0), false) => Err(ParseError::shape(TOOL, "exit 0 with findings")),
        (Some(1), true) => Err(ParseError::shape(TOOL, "exit 1 with no diagnostics")),
        _ => Ok(findings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[&str] = &["CMakeLists.txt", "sub/a.cmake", "w:x/b.cmake"];
    const DIRTY: &str = "CMakeLists.txt:2: Use lowercase commands [readability/wonkycase]\nsub/a.cmake:0: Unexpected carriage return found; better to use only \\n [whitespace/newline]\n";

    fn run(stdout: &str, stderr: &str, code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_cmakelint(stdout.as_bytes(), stderr.as_bytes(), code, FILES)
    }

    #[test]
    fn cmakelint_clean_run_is_clean() {
        assert!(run("", "Total Errors: 0\n", Some(0))
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn cmakelint_reports_line_only_findings() {
        let findings = run(DIRTY, "Total Errors: 2\n", Some(1)).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "CMakeLists.txt");
        assert_eq!(findings[0].finding.rule_id, "readability/wonkycase");
        assert_eq!(findings[0].finding.message, "Use lowercase commands");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].file, "sub/a.cmake");
        assert_eq!(findings[1].finding.start.line, 1);
    }

    #[test]
    fn cmakelint_matches_paths_against_known_files() {
        let findings =
            run("w:x/b.cmake:3: m [a/b]\n", "Total Errors: 1\n", Some(1)).expect("parsed");
        assert_eq!(findings[0].file, "w:x/b.cmake");
        assert!(run("other.cmake:3: m [a/b]\n", "Total Errors: 1\n", Some(1)).is_err());
    }

    #[test]
    fn cmakelint_requires_a_matching_total() {
        assert!(run("", "", Some(0)).is_err());
        assert!(run(DIRTY, "", Some(1)).is_err());
        assert!(run(DIRTY, "Total Errors: 1\n", Some(1)).is_err());
        assert!(run(DIRTY, "Total Errors: x\n", Some(1)).is_err());
        assert!(run("", "Total Errors: 0\nTotal Errors: 0\n", Some(0)).is_err());
        assert!(run("", "Total Errors: 3\n", Some(0)).is_err());
    }

    #[test]
    fn cmakelint_exit_code_must_agree_with_findings() {
        assert!(run(DIRTY, "Total Errors: 2\n", Some(0)).is_err());
        assert!(run("", "Total Errors: 0\n", Some(1)).is_err());
    }

    #[test]
    fn cmakelint_rejects_other_exit_codes_and_crashes() {
        assert!(run("", "Total Errors: 0\n", Some(32)).is_err());
        assert!(run("", "Total Errors: 0\n", Some(2)).is_err());
        assert!(run("", "Total Errors: 0\n", None).is_err());
        let crash = "Traceback (most recent call last):\n  boom\nTotal Errors: 0\n";
        assert!(run("", crash, Some(0)).is_err());
    }

    #[test]
    fn cmakelint_ignored_files_and_odd_lines_are_errors() {
        assert!(run("Ignoring file: x.txt\n", "Total Errors: 0\n", Some(0)).is_err());
        assert!(run(
            "Exception occurred while processing 'CMakeLists.txt:1':\n",
            "Total Errors: 0\n",
            Some(0)
        )
        .is_err());
        assert!(run("CMakeLists.txt:x: m [a/b]\n", "Total Errors: 1\n", Some(1)).is_err());
        assert!(run("CMakeLists.txt:1: m\n", "Total Errors: 1\n", Some(1)).is_err());
        assert!(run("\n", "Total Errors: 0\n", Some(0)).is_err());
        assert!(parse_cmakelint(&[0xff], b"Total Errors: 0\n", Some(0), FILES).is_err());
    }
}
