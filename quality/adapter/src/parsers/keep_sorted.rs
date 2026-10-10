use serde::Deserialize;

use super::{
    check_output_size, finding, known, lines, located, missing, numbered, require_findings,
    FileFinding, ParseError,
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

#[derive(Debug, Deserialize)]
struct KeepSortedLines {
    start: u64,
    end: u64,
}

#[derive(Debug, Deserialize)]
struct KeepSortedReport {
    path: String,
    lines: KeepSortedLines,
    message: String,
}

pub fn parse_keep_sorted_lint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "keep_sorted";
    check_output_size(TOOL, stdout)?;
    if stdout.iter().all(|byte| byte.is_ascii_whitespace()) {
        require_findings(TOOL, &[], code)?;
        return Ok(Vec::new());
    }
    let reports: Vec<KeepSortedReport> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::new();
    for report in &reports {
        if report.message.trim().is_empty() {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: "finding with empty message".to_owned(),
            });
        }
        if report.lines.start < 1 || report.lines.end < report.lines.start {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("bad lines {}:{}", report.lines.start, report.lines.end),
            });
        }
        let checked = known(TOOL, files, &report.path)?;
        findings.push(finding(
            TOOL,
            checked,
            String::new(),
            report.message.clone(),
            ToolSeverity::Warning,
            report.lines.start,
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
    const DIRECT_DIRTY: &str = "[\n  {\n    \"path\": \"notes.txt\",\n    \"lines\": {\n      \"start\": 2,\n      \"end\": 4\n    },\n    \"message\": \"These lines are out of order.\",\n    \"fixes\": [\n      {\n        \"replacements\": [\n          {\n            \"lines\": {\n              \"start\": 2,\n              \"end\": 4\n            },\n            \"new_content\": \"a\\nb\\nc\\n\"\n          }\n        ]\n      }\n    ]\n  }\n]\n";
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

    #[test]
    fn keep_sorted_lint_reports_direct_json_blocks() {
        let findings = parse_keep_sorted_lint(DIRECT_DIRTY.as_bytes(), Some(1), &["notes.txt"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "notes.txt");
        assert_eq!(findings[0].finding.tool_id, "keep_sorted");
        assert_eq!(findings[0].finding.message, "These lines are out of order.");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
        let clean = parse_keep_sorted_lint(b"", Some(0), &["notes.txt"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_keep_sorted_lint(b"", Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted_lint(b"{<error", Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted_lint(DIRECT_DIRTY.as_bytes(), Some(1), &["other.txt"]).is_err());
        assert!(parse_keep_sorted_lint(&[0xff], Some(1), &["x"]).is_err());
        assert!(parse_keep_sorted_lint(DIRTY.as_bytes(), Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted(DIRECT_DIRTY.as_bytes(), Some(1), &["notes.txt"]).is_err());
    }
}
