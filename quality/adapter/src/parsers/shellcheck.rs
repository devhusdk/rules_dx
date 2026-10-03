use super::{
    columned, finding, lines, located, missing, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_shellcheck(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "shellcheck";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let (line_no, column, tail) = columned(TOOL, line, rest)?;
        let (level, rest) = tail
            .split_once(':')
            .ok_or_else(|| missing(TOOL, "level", line))?;
        let severity = match level.trim() {
            "error" => ToolSeverity::Error,
            "warning" => ToolSeverity::Warning,
            "info" | "note" | "style" => ToolSeverity::Info,
            _ => return Err(missing(TOOL, "level", line)),
        };
        let rest = rest.trim();
        let (message, rule_id) = match rest.rsplit_once('[').and_then(|(message, rule)| {
            rule.strip_suffix(']')
                .map(|rule| (message.trim().to_owned(), rule.trim().to_owned()))
        }) {
            Some((message, rule)) if !message.is_empty() && !rule.is_empty() => (message, rule),
            _ => (rest.to_owned(), String::new()),
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
    const DIRTY: &str = "run.sh:3:1: warning: Double quote to prevent globbing [SC2086]\n";
    #[test]
    fn shellcheck_reports_gcc_lines() {
        let findings = parse_shellcheck(DIRTY.as_bytes(), Some(1), &["run.sh"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "SC2086");
        let clean = parse_shellcheck(b"", Some(0), &["run.sh"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_shellcheck(b"", Some(1), &["run.sh"]).is_err());
        assert!(parse_shellcheck(DIRTY.as_bytes(), Some(1), &["other.sh"]).is_err());
        assert!(parse_shellcheck(&[0xff], Some(1), &["x"]).is_err());
    }
}
