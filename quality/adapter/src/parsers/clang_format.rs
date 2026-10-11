use super::{FileFinding, ParseError, as_text, code_name, finding, known, missing, numbered};
use crate::ToolSeverity;

pub fn parse_clang_format(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_format";
    let text = as_text(TOOL, stderr)?;
    let mut findings = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((path, rest)) = split_reported(line, files) else {
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
        let checked = known(TOOL, files, path)?;
        findings.push(finding(
            TOOL, checked, rule_id, message, severity, line_no, column,
        ));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
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

    const DIRTY: &str = "cc/tests/fixtures/clang_format/Sample.c:1:11: error: code should be clang-formatted [-Wclang-format-violations]\nint greet( const char*name){return 0;}\n          ^\ncc/tests/fixtures/clang_format/Sample.c:1:22: error: code should be clang-formatted [-Wclang-format-violations]\n";

    #[test]
    fn clang_format_reports_stderr_diagnostics() {
        let findings = parse_clang_format(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/clang_format/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_format/Sample.c");
        assert_eq!(findings[0].finding.message, "code should be clang-formatted");
        assert_eq!(findings[0].finding.rule_id, "-Wclang-format-violations");
        assert_eq!((findings[0].finding.start.line, findings[0].finding.start.column), (1, 11));
        assert_eq!((findings[1].finding.start.line, findings[1].finding.start.column), (1, 22));
        let clean = parse_clang_format(b"", Some(0), &["cc/tests/fixtures/clang_format/Sample.c"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_clang_format(b"", Some(1), &["cc/tests/fixtures/clang_format/Sample.c"]).is_err()
        );
        assert!(parse_clang_format(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_clang_format(&[0xff], Some(1), &["x"]).is_err());
    }
}
