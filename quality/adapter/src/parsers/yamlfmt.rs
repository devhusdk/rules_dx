use super::{as_text, code_name, known, point, FileFinding, ParseError};
use crate::{Finding, ToolSeverity};

/// The header `yamlfmt -lint -q` prints above the paths it would rewrite.
const HEADER: &str = "The following files had formatting differences:";

pub fn parse_yamlfmt(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "yamlfmt";
    let text = as_text(TOOL, stderr)?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == HEADER {
            continue;
        }
        if !paths.contains(&trimmed.to_owned()) {
            paths.push(trimmed.to_owned());
        }
    }
    if paths.is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no paths", code_name(code)),
        });
    }
    let mut findings = Vec::with_capacity(paths.len());
    for path in paths {
        let checked = known(TOOL, files, &path)?;
        let (start, end) = point(1, 1);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: String::new(),
                message: "file is not formatted".to_owned(),
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `yamlfmt -lint -q` output, verbatim from yamlfmt 0.21.0 on stderr.
    const DIRTY: &str =
        "The following files had formatting differences:\n\nSample.yaml\nOther.yaml\n";

    #[test]
    fn yamlfmt_reports_listed_files() {
        let findings = parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["Sample.yaml", "Other.yaml"])
            .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Sample.yaml");
        assert_eq!(findings[1].file, "Other.yaml");
        let clean = parse_yamlfmt(b"", Some(0), &["Sample.yaml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_yamlfmt(b"", Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamlfmt(&[0xff], Some(0), &["x"]).is_err());
    }

    /// `yamlfmt -lint` writes the long side-by-side report to stderr, never a unified diff.
    #[test]
    fn yamlfmt_writes_its_report_to_stderr() {
        let report = b"The following formatting differences were found:\n\nSample.yaml:\n- key:    value  key: value\n                 \n\n";
        assert!(parse_yamlfmt(report, Some(1), &["Sample.yaml"]).is_err());
    }
}
