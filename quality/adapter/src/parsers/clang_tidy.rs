use super::{
    as_text, finding, missing, numbered, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_clang_tidy(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_tidy";
    let text = as_text(TOOL, stdout)?;
    let mut findings = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((file, rest)) = split_reported(line, files) else {
            continue;
        };
        let rest = rest.trim_start_matches(':').trim_start();
        let mut parts = rest.splitn(4, ':');
        let line_no = numbered(TOOL, "line", line, parts.next())?;
        let column = numbered(TOOL, "column", line, parts.next())?;
        let severity_word = parts
            .next()
            .ok_or_else(|| missing(TOOL, "severity", line))?
            .trim();
        let severity = match severity_word {
            "warning" => ToolSeverity::Warning,
            "error" => ToolSeverity::Error,
            _ => return Err(missing(TOOL, "severity", line)),
        };
        let message = parts
            .next()
            .ok_or_else(|| missing(TOOL, "message", line))?
            .trim();
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

fn split_reported<'a, 'b>(line: &'a str, files: &[&'b str]) -> Option<(&'b str, &'a str)> {
    for file in files {
        if let Some(rest) = line.strip_prefix(file) {
            if rest.starts_with(':') {
                return Some((file, rest));
            }
        }
    }
    None
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

    const SNIPPET: &str = "cc/tests/fixtures/clang_tidy/Sample.c:4:5: warning: do not use 'else' after 'return' [readability-else-after-return]\n    4 |   } else {\n      |     ^~~~~~\n    5 |     return 0;\n      |     ~\n";

    #[test]
    fn clang_tidy_skips_source_snippets_and_accepts_exit_zero() {
        let findings = parse_clang_tidy(
            SNIPPET.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/clang_tidy/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_tidy/Sample.c");
        assert_eq!(findings[0].finding.rule_id, "readability-else-after-return");
        assert_eq!(
            (findings[0].finding.start.line, findings[0].finding.start.column),
            (4, 5)
        );
    }
}
