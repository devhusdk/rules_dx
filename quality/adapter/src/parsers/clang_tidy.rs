use super::{finding, known, lines, missing, require_findings, FileFinding, ParseError};
use crate::ToolSeverity;

fn split_diagnostic(line: &str) -> Option<(&str, &str, &str, &str, &str)> {
    let (file, rest) = line.split_once(':')?;
    let rest = rest.trim_start_matches(':').trim_start();
    let mut parts = rest.splitn(4, ':');
    let line_no = parts.next()?.trim();
    let column = parts.next()?.trim();
    if line_no.is_empty() || !line_no.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if column.is_empty() || !column.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let severity = match parts.next()?.trim() {
        word @ ("warning" | "error" | "note") => word,
        _ => return None,
    };
    Some((file, line_no, column, severity, parts.next()?.trim()))
}

pub fn parse_clang_tidy(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_tidy";
    let mut findings = Vec::new();
    for line in lines(TOOL, stderr)? {
        let Some((path, line_no, column, severity_word, message)) = split_diagnostic(line)
        else {
            if files
                .iter()
                .any(|file| line.starts_with(*file) && line[file.len()..].starts_with(':'))
            {
                return Err(missing(TOOL, "diagnostic", line));
            }
            continue;
        };
        let file = known(TOOL, files, path)?;
        if severity_word == "note" {
            continue;
        }
        let severity = match severity_word {
            "warning" => ToolSeverity::Warning,
            "error" => ToolSeverity::Error,
            _ => return Err(missing(TOOL, "severity", line)),
        };
        let line_no = line_no
            .parse()
            .map_err(|_| missing(TOOL, "line", line))?;
        let column = column
            .parse()
            .map_err(|_| missing(TOOL, "column", line))?;
        if message.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        let (message, rule_id) = match message.rfind(" [") {
            Some(start) if message.ends_with(']') => (
                message[..start].trim_end().to_owned(),
                message[start + 2..message.len() - 1].to_owned(),
            ),
            _ => (message.to_owned(), String::new()),
        };
        if message.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        findings.push(finding(
            TOOL, file, rule_id, message, severity, line_no, column,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "cc/tests/fixtures/clang_tidy/Sample.c:4:3: warning: do not use 'else' after 'return' [readability-else-after-return]\n";

    #[test]
    fn clang_tidy_reports_text_diagnostics() {
        let findings = parse_clang_tidy(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/clang_tidy/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_tidy/Sample.c");
        assert_eq!(findings[0].finding.rule_id, "readability-else-after-return");
        assert_eq!(
            findings[0].finding.message,
            "do not use 'else' after 'return'"
        );
        let clean = parse_clang_tidy(b"", Some(0), &["cc/tests/fixtures/clang_tidy/Sample.c"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_clang_tidy(b"", Some(1), &["cc/tests/fixtures/clang_tidy/Sample.c"]).is_err()
        );
        assert!(parse_clang_tidy(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_clang_tidy(b"nope\n", Some(1), &["x"]).is_err());
        assert!(parse_clang_tidy(&[0xff], Some(1), &["x"]).is_err());
    }

    const REAL_WAE_DIRTY: &str = "matrix/clang_tidy_dirty.c:7:5: error: do not use 'else' after 'return' [readability-else-after-return,-warnings-as-errors]\n    7 |   } else {\n      |     ^~~~~~\n    8 |     return 0;\n    9 |   }\n      |   ~\n";

    #[test]
    fn clang_tidy_reads_measured_warning_as_errors_output() {
        let findings = parse_clang_tidy(
            REAL_WAE_DIRTY.as_bytes(),
            Some(1),
            &["matrix/clang_tidy_dirty.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "matrix/clang_tidy_dirty.c");
        assert_eq!(
            findings[0].finding.rule_id,
            "readability-else-after-return,-warnings-as-errors"
        );
        assert_eq!(
            findings[0].finding.message,
            "do not use 'else' after 'return'"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 7);
        assert_eq!(findings[0].finding.start.column, 5);
        assert!(
            parse_clang_tidy(REAL_WAE_DIRTY.as_bytes(), Some(1), &["other.c"]).is_err()
        );
    }

    const REAL_WARNING_DIRTY: &str = "matrix/clang_tidy_dirty.c:7:5: warning: do not use 'else' after 'return' [readability-else-after-return]\n    7 |   } else {\n      |     ^~~~~~\n    8 |     return 0;\n    9 |   }\n      |   ~\n";

    #[test]
    fn clang_tidy_accepts_warnings_with_a_zero_exit() {
        let findings = parse_clang_tidy(
            REAL_WARNING_DIRTY.as_bytes(),
            Some(0),
            &["matrix/clang_tidy_dirty.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.rule_id,
            "readability-else-after-return"
        );
    }

    const REAL_NOTE_DIRTY: &str = "src/move.cpp:5:10: warning: 's' used after it was moved [bugprone-use-after-move]\n    5 |   return s;\n      |          ^\nsrc/move.cpp:4:19: note: move occurred here\n    4 |   std::string t = std::move(s);\n      |                   ^\n";

    #[test]
    fn clang_tidy_skips_note_lines_and_colon_echo_lines() {
        let findings =
            parse_clang_tidy(REAL_NOTE_DIRTY.as_bytes(), Some(0), &["src/move.cpp"])
                .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].finding.rule_id,
            "bugprone-use-after-move"
        );
        assert!(
            parse_clang_tidy(REAL_NOTE_DIRTY.as_bytes(), Some(0), &["other.cpp"]).is_err()
        );
    }

    #[test]
    fn clang_tidy_rejects_anchored_malformed_lines() {
        assert!(
            parse_clang_tidy(b"a.c:4: bogus\n", Some(1), &["a.c"]).is_err()
        );
        assert!(
            parse_clang_tidy(b"a.c:4:3: bogus: nope\n", Some(1), &["a.c"]).is_err()
        );
    }
}
