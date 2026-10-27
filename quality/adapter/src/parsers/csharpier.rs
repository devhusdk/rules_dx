use super::{as_text, code_name, finding, known, row_col, unformatted_at, FileFinding, ParseError};
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
    let mut syntax: Option<(&str, usize)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Error ") {
            close_syntax(&mut syntax, &mut findings);
            if let Some(path) = rest.strip_suffix(UNFORMATTED) {
                let path = path.strip_prefix("./").unwrap_or(path);
                findings.push(unformatted_at(TOOL, known(TOOL, files, path)?, 1, 1));
            } else if let Some(path) = rest.strip_suffix(SYNTAX) {
                let path = path.strip_prefix("./").unwrap_or(path);
                syntax = Some((known(TOOL, files, path)?, findings.len()));
            } else {
                return Err(ParseError::shape(
                    TOOL,
                    format!("unrecognized error line: {line}"),
                ));
            }
        } else if line.starts_with(char::is_whitespace) || line.trim().is_empty() {
            if let Some((file, _)) = syntax {
                if let Some(found) = syntax_error(file, line.trim()) {
                    findings.push(found);
                }
            }
        } else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognized line: {line}"),
            ));
        }
    }
    close_syntax(&mut syntax, &mut findings);
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with no files", code_name(code)),
        ));
    }
    Ok(findings)
}

fn close_syntax(syntax: &mut Option<(&str, usize)>, findings: &mut Vec<FileFinding>) {
    if let Some((file, before)) = syntax.take() {
        if findings.len() == before {
            findings.push(finding(
                TOOL,
                file,
                "syntax".to_owned(),
                "was not formatted due to syntax errors".to_owned(),
                ToolSeverity::Error,
                1,
                1,
            ));
        }
    }
}

fn syntax_error(file: &str, line: &str) -> Option<FileFinding> {
    let rest = line.strip_prefix('(')?;
    let (position, message) = rest.split_once("): error ")?;
    let (row, column) = row_col(position, ',')?;
    let (rule, text) = message.split_once(": ")?;
    Some(finding(
        TOOL,
        file,
        rule.to_owned(),
        text.to_owned(),
        ToolSeverity::Error,
        row,
        column,
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
    fn csharpier_keeps_a_syntax_error_file_without_detail_lines() {
        let stderr = "Error /s/Sample.cs - Was not formatted due to syntax errors.\nError /s/Other.cs - Was not formatted.\n";
        let findings =
            parse_csharpier(stderr.as_bytes(), Some(1), &[FILE, "/s/Other.cs"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[1].file, "/s/Other.cs");
        let tail = "Error /s/Sample.cs - Was not formatted due to syntax errors.\n  odd\n";
        let kept = parse_csharpier(tail.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn csharpier_rejects_stray_output_beside_findings() {
        let stderr = "Error /s/Sample.cs - Was not formatted.\nBoom\n";
        assert!(parse_csharpier(stderr.as_bytes(), Some(1), &[FILE]).is_err());
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
