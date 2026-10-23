use super::{as_text, code_name, finding, known, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;

const TOOL: &str = "csharpier";
const UNFORMATTED: &str = " - Was not formatted.";
const SYNTAX: &str = " - Was not formatted due to syntax errors.";

pub fn parse_csharpier(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let mut findings = Vec::new();
    let mut syntax_file: Option<&str> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Error ") {
            syntax_file = None;
            if let Some(path) = rest.strip_suffix(UNFORMATTED) {
                let path = path.strip_prefix("./").unwrap_or(path);
                findings.push(unformatted_at(TOOL, known(TOOL, files, path)?, 1, 1));
            } else if let Some(path) = rest.strip_suffix(SYNTAX) {
                let path = path.strip_prefix("./").unwrap_or(path);
                syntax_file = Some(known(TOOL, files, path)?);
            } else {
                return Err(shape(format!("unrecognized error line: {line}")));
            }
        } else if let Some(file) = syntax_file {
            if let Some(found) = syntax_error(file, line.trim()) {
                findings.push(found);
            }
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(shape(format!("exit {} with no files", code_name(code))));
    }
    Ok(findings)
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

fn syntax_error(file: &str, line: &str) -> Option<FileFinding> {
    let rest = line.strip_prefix('(')?;
    let (position, message) = rest.split_once("): error ")?;
    let (row, column) = position.split_once(',')?;
    let (rule, text) = message.split_once(": ")?;
    Some(finding(
        TOOL,
        file,
        rule.to_owned(),
        text.to_owned(),
        ToolSeverity::Error,
        row.parse().ok()?,
        column.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/Sample.cs";

    const DIRTY: &str = "Error /s/Sample.cs - Was not formatted.\n  ----------------------------- Expected: Around Line 1 -----------------------------\n  class A\n  {\n  ----------------------------- Actual: Around Line 1 -----------------------------\n  class  A{int x ;}\n  \n";

    #[test]
    fn csharpier_reports_unformatted_files() {
        let findings =
            parse_csharpier(DIRTY.as_bytes(), Some(1), &[FILE, "/s/Other.cs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_csharpier(b"", Some(0), &[FILE]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_csharpier(b"", Some(1), &[FILE]).is_err());
        assert!(parse_csharpier(DIRTY.as_bytes(), Some(1), &["/s/Other.cs"]).is_err());
        assert!(parse_csharpier(&[0xff], Some(1), &[FILE]).is_err());
    }

    #[test]
    fn csharpier_reports_syntax_errors_with_positions() {
        let stderr = "Error /s/Sample.cs - Was not formatted due to syntax errors.\n  (1,10): error CS1519: Invalid token '{' in a member declaration\n  (1,12): error CS1513: } expected\n";
        let findings = parse_csharpier(stderr.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.rule_id, "CS1519");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.column, 10);
        assert_eq!(findings[1].finding.message, "} expected");
    }

    #[test]
    fn csharpier_accepts_a_path_spelled_below_the_dot() {
        let findings = parse_csharpier(
            b"Error ./csharpier/Sample.cs - Was not formatted.\n",
            Some(1),
            &["csharpier/Sample.cs"],
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "csharpier/Sample.cs");
    }

    #[test]
    fn csharpier_rejects_an_unknown_error_line() {
        assert!(parse_csharpier(b"Error /s/Sample.cs - Boom.\n", Some(1), &[FILE]).is_err());
        assert!(parse_csharpier(
            b"There was no file or directory found at x\n",
            Some(1),
            &[FILE]
        )
        .is_err());
    }
}
