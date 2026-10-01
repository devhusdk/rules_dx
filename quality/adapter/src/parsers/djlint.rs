use super::{
    check_output_size, code_name, diff_format, known, missing, point, DiffExit, FileFinding,
    ParseError,
};
use crate::{Finding, ToolSeverity};

pub fn parse_djlint(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "djlint";
    check_output_size(TOOL, stderr)?;
    let text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (path, rest) = trimmed
            .split_once(':')
            .ok_or_else(|| missing(TOOL, "location", line))?;
        let checked = known(TOOL, files, path)?;
        let mut parts = rest.splitn(3, ':');
        let line_no: u64 = parts
            .next()
            .ok_or_else(|| missing(TOOL, "line", line))?
            .trim()
            .parse()
            .map_err(|_| missing(TOOL, "line", line))?;
        let col_no: u64 = parts
            .next()
            .ok_or_else(|| missing(TOOL, "column", line))?
            .trim()
            .parse()
            .map_err(|_| missing(TOOL, "column", line))?;
        let tail = parts
            .next()
            .ok_or_else(|| missing(TOOL, "message", line))?
            .trim();
        if tail.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        let (rule, message) = match tail.split_once(' ') {
            Some((r, m)) if !r.is_empty() && !m.is_empty() => (r.to_owned(), m.to_owned()),
            _ => (String::new(), tail.to_owned()),
        };
        let (start, end) = point(line_no, col_no);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule,
                message,
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
    Ok(findings)
}

pub fn parse_djlint_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("djlint", stdout, code, files, DiffExit::NonZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    const LINT_DIRTY: &str = "base.html:3:1: H006 img tags require alt text\n";
    const DIRTY: &str = "--- base.html.orig\n+++ base.html\n@@ -1 +1 @@\n-BADFMT\n+fixed";
    #[test]
    fn djlint_reports_lint_and_format() {
        let findings =
            parse_djlint(LINT_DIRTY.as_bytes(), Some(1), &["base.html"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "H006");
        let clean = parse_djlint(b"", Some(0), &["base.html"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_djlint(b"", Some(1), &["base.html"]).is_err());
        let fmt = parse_djlint_format(DIRTY.as_bytes(), Some(1), &["base.html"]).expect("parsed");
        assert_eq!(fmt.len(), 1);
        let fmt_clean = parse_djlint_format(b"", Some(0), &["base.html"]).expect("parsed");
        assert!(fmt_clean.is_empty());
        assert!(parse_djlint(&[0xff], Some(1), &["x"]).is_err());
    }
}
