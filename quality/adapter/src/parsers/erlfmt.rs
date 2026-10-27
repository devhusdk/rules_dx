use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{
    as_text, code_name, finding, known, prefixed, unformatted_at, FileFinding, ParseError,
};

const TOOL: &str = "erlfmt";

const NOISE: &[&str] = &[
    "Checking formatting...",
    "All matched files use erlfmt code style!",
    "[warn] Code style issues found in the above file(s). Forgot to run erlfmt?",
];

/// Findings from `erlfmt --check`: `[warn] path` lines are unformatted files, `path:line:col: message` lines are errors.
pub fn parse_erlfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !as_text(TOOL, stdout)?.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "--check wrote to stdout"));
    }
    let text = as_text(TOOL, stderr)?;
    let mut unformatted: Vec<FileFinding> = Vec::new();
    let mut errors = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || NOISE.contains(&line) {
            continue;
        }
        if let Some(path) = line.strip_prefix("[warn] ") {
            let checked = known(TOOL, files, path)?;
            if !unformatted.iter().any(|found| found.file == checked) {
                unformatted.push(unformatted_at(TOOL, checked, 1, 1, ColumnEncoding::Scalars));
            }
            continue;
        }
        if let Some(error) = located_error(line, files) {
            errors.push(error);
        }
    }
    let shaped = match code {
        Some(0) => unformatted.is_empty(),
        Some(1) => !unformatted.is_empty(),
        Some(4) => !errors.is_empty(),
        _ => false,
    };
    if !shaped {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} with {} unformatted files and {} errors",
                code_name(code),
                unformatted.len(),
                errors.len()
            ),
        ));
    }
    unformatted.extend(errors);
    Ok(unformatted)
}

fn located_error(line: &str, files: &[&str]) -> Option<FileFinding> {
    let (file, rest) = prefixed(line, files)?;
    let mut parts = rest.splitn(3, ':');
    let first = parts.next().and_then(|part| part.parse::<u64>().ok());
    let second = parts.next();
    let (row, col, message, rule) = match (first, second.and_then(|part| part.parse().ok())) {
        (Some(row), Some(col)) => (row, col, parts.next().unwrap_or(""), "syntax"),
        (Some(row), None) => (
            row,
            1,
            rest.split_once(':').map_or("", |(_, tail)| tail),
            "syntax",
        ),
        (None, _) => (1, 1, rest, "file"),
    };
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    Some(finding(
        TOOL,
        file,
        rule.to_owned(),
        message.to_owned(),
        ToolSeverity::Error,
        TextPosition {
            line: row,
            column: col,
        },
        ColumnEncoding::Scalars,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const DIRTY: &str = "Checking formatting...\n[warn] src/dirty.erl\n[warn] Code style issues found in the above file(s). Forgot to run erlfmt?\n";
    const CLEAN: &str = "Checking formatting...\nAll matched files use erlfmt code style!\n";
    const SKIPPED_FORM: &str = "Checking formatting...\nsrc/bad.erl:2:12: syntax error before: '.'\nAll matched files use erlfmt code style!\n";
    const FATAL: &str =
        "Checking formatting...\nsrc/bad.erl:2:10: unterminated string starting with \"x\\n\"\n";

    #[test]
    fn erlfmt_reports_listed_files() {
        let files = ["src/dirty.erl", "src/clean.erl"];
        let findings = parse_erlfmt(b"", DIRTY.as_bytes(), Some(1), &files).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/dirty.erl");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert!(parse_erlfmt(b"", CLEAN.as_bytes(), Some(0), &files)
            .expect("parsed")
            .is_empty());
        assert!(parse_erlfmt(b"", CLEAN.as_bytes(), Some(1), &files).is_err());
        assert!(parse_erlfmt(b"", DIRTY.as_bytes(), Some(0), &files).is_err());
        assert!(matches!(
            parse_erlfmt(b"", b"[warn] other.erl\n", Some(1), &files),
            Err(ParseError::UnknownFile { path, .. }) if path == "other.erl"
        ));
        assert!(parse_erlfmt(b"-module(x).\n", CLEAN.as_bytes(), Some(0), &files).is_err());
        assert!(parse_erlfmt(b"", &[0xff], Some(0), &files).is_err());
    }

    #[test]
    fn erlfmt_reports_a_skipped_form_even_when_the_exit_is_clean() {
        let files = ["src/bad.erl"];
        let findings = parse_erlfmt(b"", SKIPPED_FORM.as_bytes(), Some(0), &files).expect("parsed");
        assert_eq!(findings.len(), 1);
        let error = &findings[0].finding;
        assert_eq!(findings[0].file, "src/bad.erl");
        assert_eq!(error.severity, ToolSeverity::Error);
        assert_eq!(error.rule_id, "syntax");
        assert_eq!(error.message, "syntax error before: '.'");
        assert_eq!(
            error.start,
            TextPosition {
                line: 2,
                column: 12
            }
        );
    }

    #[test]
    fn erlfmt_reports_fatal_errors_and_rejects_unpinned_exits() {
        let files = ["src/bad.erl", "src/dirty.erl"];
        let findings = parse_erlfmt(b"", FATAL.as_bytes(), Some(4), &files).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].finding.message,
            "unterminated string starting with \"x\\n\""
        );
        let read = "src/dirty.erl: no such file or directory\n";
        let findings = parse_erlfmt(b"", read.as_bytes(), Some(4), &files).expect("parsed");
        assert_eq!(findings[0].finding.rule_id, "file");
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 1, column: 1 }
        );
        let lined = "src/bad.erl:7: bad attribute\n";
        let findings = parse_erlfmt(b"", lined.as_bytes(), Some(4), &files).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 7);
        assert_eq!(findings[0].finding.message, "bad attribute");
        assert!(parse_erlfmt(b"", b"Checking formatting...\n", Some(4), &files).is_err());
        assert!(parse_erlfmt(b"", FATAL.as_bytes(), Some(2), &files).is_err());
        assert!(parse_erlfmt(b"", b"erlfmt Internal Error\n", Some(127), &files).is_err());
        assert!(parse_erlfmt(b"", FATAL.as_bytes(), None, &files).is_err());
        assert!(parse_erlfmt(b"", FATAL.as_bytes(), Some(4), &["src/other.erl"]).is_err());
    }
}
