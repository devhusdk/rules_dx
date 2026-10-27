use super::{
    as_text, check_output_size, code_name, finding, known, located, row_col, unformatted_at,
    FileFinding, ParseError,
};
use crate::ToolSeverity;

const TOOL: &str = "remark";

/// Reads `remark.mjs --check`: a JSON array of `{file, formatted}` on stdout, and on exit 2
/// one `path:line:col: reason` line per file it could not format on stderr.
pub fn parse_remark(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    if !matches!(code, Some(0..=2)) {
        return Err(ParseError::shape(TOOL, format!("exit {}", code_name(code))));
    }
    let mut findings = unformatted(stdout, files)?;
    if code == Some(2) {
        let before = findings.len();
        for line in as_text(TOOL, stderr)?.lines() {
            if line.trim().is_empty() {
                continue;
            }
            findings.push(error_line(line, files)?);
        }
        if findings.len() == before {
            return Err(ParseError::shape(TOOL, "exit 2 with no located error"));
        }
    } else if findings.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} with {} unformatted files",
                code_name(code),
                findings.len()
            ),
        ));
    }
    Ok(findings)
}

fn unformatted(stdout: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let entries = value
        .as_array()
        .ok_or_else(|| ParseError::shape(TOOL, "expected JSON array"))?;
    let mut findings = Vec::new();
    for entry in entries {
        let file = entry
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::shape(TOOL, "missing file"))?;
        let checked = known(TOOL, files, file)?;
        let formatted = entry
            .get("formatted")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| ParseError::shape(TOOL, format!("missing formatted for {file}")))?;
        if !formatted {
            findings.push(unformatted_at(TOOL, checked, 1, 1));
        }
    }
    Ok(findings)
}

fn error_line(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let (file, rest) = located(TOOL, line, files)?;
    let (position, reason) = rest
        .split_once(": ")
        .ok_or_else(|| ParseError::shape(TOOL, format!("malformed error line: {line}")))?;
    let (row, column) = row_col(position, ':')
        .ok_or_else(|| ParseError::shape(TOOL, format!("malformed location: {line}")))?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(ParseError::shape(TOOL, format!("missing reason: {line}")));
    }
    Ok(finding(
        TOOL,
        file,
        "syntax".to_owned(),
        reason.to_owned(),
        ToolSeverity::Error,
        row,
        column,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/docs/a.mdx";
    const OTHER: &str = "/s/docs/b.mdx";

    #[test]
    fn remark_reports_each_unformatted_file_at_its_start() {
        let stdout = br#"[{"file":"/s/docs/a.mdx","formatted":false},{"file":"/s/docs/b.mdx","formatted":true}]"#;
        let findings = parse_remark(stdout, b"", Some(1), &[FILE, OTHER]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 1);
        let clean = br#"[{"file":"/s/docs/a.mdx","formatted":true}]"#;
        assert!(parse_remark(clean, b"", Some(0), &[FILE])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn remark_holds_the_exit_code_to_the_report() {
        let clean = br#"[{"file":"/s/docs/a.mdx","formatted":true}]"#;
        assert!(parse_remark(clean, b"", Some(1), &[FILE]).is_err());
        let dirty = br#"[{"file":"/s/docs/a.mdx","formatted":false}]"#;
        assert!(parse_remark(dirty, b"", Some(0), &[FILE]).is_err());
        assert!(parse_remark(clean, b"", Some(3), &[FILE]).is_err());
        assert!(parse_remark(clean, b"", None, &[FILE]).is_err());
    }

    #[test]
    fn remark_reports_a_parse_error_where_it_happened() {
        let stderr = b"/s/docs/a.mdx:5:1: Expected a closing tag for `<Chart>` (5:1-5:25)\n";
        let findings = parse_remark(b"[]\n", stderr, Some(2), &[FILE]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[0].finding.start.line, 5);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(
            findings[0].finding.message,
            "Expected a closing tag for `<Chart>` (5:1-5:25)"
        );
    }

    #[test]
    fn remark_keeps_unformatted_files_beside_a_parse_error() {
        let stdout = br#"[{"file":"/s/docs/b.mdx","formatted":false}]"#;
        let stderr = b"/s/docs/a.mdx:1:1: would change the document tree\n";
        let findings = parse_remark(stdout, stderr, Some(2), &[FILE, OTHER]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, OTHER);
        assert_eq!(findings[1].file, FILE);
        assert_eq!(
            findings[1].finding.message,
            "would change the document tree"
        );
    }

    #[test]
    fn remark_rejects_output_outside_the_grammar() {
        assert!(parse_remark(b"not json", b"", Some(0), &[FILE]).is_err());
        assert!(parse_remark(b"{}", b"", Some(0), &[FILE]).is_err());
        assert!(parse_remark(
            br#"[{"file":"/s/x.mdx","formatted":false}]"#,
            b"",
            Some(1),
            &[FILE]
        )
        .is_err());
        assert!(parse_remark(br#"[{"file":"/s/docs/a.mdx"}]"#, b"", Some(1), &[FILE]).is_err());
        assert!(parse_remark(b"[]", b"", Some(2), &[FILE]).is_err());
        assert!(parse_remark(
            b"[]",
            b"remark: settings r.json has keys remark-stringify does not take: plugins\n",
            Some(2),
            &[FILE]
        )
        .is_err());
        assert!(parse_remark(b"[]", b"/s/docs/a.mdx:x:1: boom\n", Some(2), &[FILE]).is_err());
        assert!(parse_remark(b"[]", b"/s/docs/a.mdx:1:1: \n", Some(2), &[FILE]).is_err());
        assert!(parse_remark(b"[]", &[0xff], Some(2), &[FILE]).is_err());
    }
}
