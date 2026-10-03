use super::{
    finding, lines, located, missing, numbered, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_keep_sorted(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "keep_sorted";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let mut parts = rest.splitn(2, ':');
        let line_no = numbered(TOOL, "line", line, parts.next())?;
        let message = parts
            .next()
            .ok_or_else(|| missing(TOOL, "message", line))?
            .trim();
        if message.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.to_owned(),
            ToolSeverity::Warning,
            line_no,
            1,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "notes.txt:4: block is not sorted\n";
    #[test]
    fn keep_sorted_reports_line_points() {
        let findings =
            parse_keep_sorted(DIRTY.as_bytes(), Some(1), &["notes.txt"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_keep_sorted(b"", Some(0), &["notes.txt"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_keep_sorted(b"", Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted(DIRTY.as_bytes(), Some(1), &["other.txt"]).is_err());
        assert!(parse_keep_sorted(&[0xff], Some(1), &["x"]).is_err());
    }
}
