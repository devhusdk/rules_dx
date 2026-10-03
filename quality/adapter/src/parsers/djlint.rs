use super::{
    columned, diff_format, finding, lines, located, missing, require_findings, DiffExit,
    FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_djlint(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "djlint";
    let mut findings = Vec::new();
    for line in lines(TOOL, stderr)? {
        let (file, rest) = located(TOOL, line, files)?;
        let (line_no, column, tail) = columned(TOOL, line, rest)?;
        if tail.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        let (rule_id, message) = match tail.split_once(' ') {
            Some((rule, message)) if !rule.is_empty() && !message.is_empty() => {
                (rule.to_owned(), message.to_owned())
            }
            _ => (String::new(), tail.to_owned()),
        };
        findings.push(finding(
            TOOL,
            file,
            rule_id,
            message,
            ToolSeverity::Warning,
            line_no,
            column,
        ));
    }
    require_findings(TOOL, &findings, code)?;
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
