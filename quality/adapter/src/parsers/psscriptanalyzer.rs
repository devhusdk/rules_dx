use super::{
    bracketed, columned, finding, lines, located, missing, require_findings, FileFinding,
    ParseError,
};
use crate::ToolSeverity;

pub fn parse_psscriptanalyzer(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "psscriptanalyzer";
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
    const DIRTY: &str = "Sample.ps1:4:1: [PSAvoidUsingWriteHost] Avoid using Write-Host\n";
    #[test]
    fn psscriptanalyzer_reports_bracketed_rule() {
        let findings =
            parse_psscriptanalyzer(DIRTY.as_bytes(), Some(1), &["Sample.ps1"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "PSAvoidUsingWriteHost");
        let clean = parse_psscriptanalyzer(b"", Some(0), &["Sample.ps1"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_psscriptanalyzer(b"", Some(1), &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(DIRTY.as_bytes(), Some(1), &["other.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(&[0xff], Some(1), &["x"]).is_err());
    }
}
