use super::{
    bracketed, columned, finding, lines, located, missing, require_findings, FileFinding,
    ParseError,
};
use crate::ToolSeverity;

pub fn parse_yamllint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "yamllint";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let (line_no, column, tail) = columned(TOOL, line, rest)?;
        if tail.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        let (rule_id, message) = bracketed(tail);
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

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "Sample.yaml:2:1: [trailing-spaces] trailing spaces\n";
    #[test]
    fn yamllint_reports_bracketed_rule() {
        let findings = parse_yamllint(DIRTY.as_bytes(), Some(2), &["Sample.yaml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "trailing-spaces");
        let clean = parse_yamllint(b"", Some(0), &["Sample.yaml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_yamllint(b"", Some(2), &["Sample.yaml"]).is_err());
        assert!(parse_yamllint(DIRTY.as_bytes(), Some(2), &["other.yaml"]).is_err());
        assert!(parse_yamllint(&[0xff], Some(2), &["x"]).is_err());
    }
}
