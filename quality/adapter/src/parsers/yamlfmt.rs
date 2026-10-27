use super::{as_text, code_name, finding, listed_paths, FileFinding, ParseError, Spelling};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "yamlfmt";
const HEADER: &str = "The following files had formatting differences:";
const ERRORS: &str = "encountered the following formatting errors:";

/// `yamlfmt -lint -q` lists unformatted files after a header and parse failures as
/// `<path>: <message>` lines after another, both on stderr.
pub fn parse_yamlfmt(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let mut listed = String::new();
    let mut failures: Vec<&str> = Vec::new();
    let mut in_errors = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == HEADER {
            in_errors = false;
        } else if trimmed == ERRORS {
            in_errors = true;
        } else if in_errors {
            if !trimmed.is_empty() {
                failures.push(trimmed);
            }
        } else {
            listed.push_str(line);
            listed.push('\n');
        }
    }
    let mut findings = listed_paths(
        TOOL,
        listed.as_bytes(),
        if failures.is_empty() { code } else { Some(0) },
        files,
        &[],
        Spelling::Exact,
    )?;
    for line in failures {
        findings.push(failure(line, files)?);
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
    Ok(findings)
}

fn failure(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let located = files.iter().find_map(|file| {
        let message = line.strip_prefix(file)?.strip_prefix(": ")?;
        Some((*file, message))
    });
    let Some((file, message)) = located else {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("error without a known file: {line}"),
        });
    };
    let row = message
        .strip_prefix("yaml: line ")
        .and_then(|rest| rest.split_once(':'))
        .and_then(|(row, _)| row.parse::<u64>().ok())
        .unwrap_or(1);
    Ok(finding(
        TOOL,
        file,
        String::new(),
        message.to_owned(),
        ToolSeverity::Error,
        TextPosition {
            line: row,
            column: 1,
        },
        ColumnEncoding::Scalars,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `yamlfmt -lint -q` output, verbatim from yamlfmt 0.21.0 on stderr.
    const DIRTY: &str =
        "The following files had formatting differences:\n\nSample.yaml\nOther.yaml\n";
    const BROKEN: &str = "encountered the following formatting errors:\nbroken.yaml: yaml: line 3: did not find expected ',' or ']'\n";

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

    #[test]
    fn yamlfmt_reports_parse_failures_with_their_line() {
        let findings = parse_yamlfmt(BROKEN.as_bytes(), Some(1), &["broken.yaml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "broken.yaml");
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(
            findings[0].finding.message,
            "yaml: line 3: did not find expected ',' or ']'"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(parse_yamlfmt(BROKEN.as_bytes(), Some(1), &["other.yaml"]).is_err());
    }

    #[test]
    fn yamlfmt_reports_both_sections_together() {
        let both = format!("{DIRTY}\n{BROKEN}");
        let findings = parse_yamlfmt(
            both.as_bytes(),
            Some(1),
            &["Sample.yaml", "Other.yaml", "broken.yaml"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 3);
    }

    /// `yamlfmt -lint -q` writes the long side-by-side report to stderr, never a unified diff.
    #[test]
    fn yamlfmt_writes_its_report_to_stderr() {
        let report = b"The following formatting differences were found:\n\nSample.yaml:\n- key:    value  key: value\n                 \n\n";
        assert!(parse_yamlfmt(report, Some(1), &["Sample.yaml"]).is_err());
    }
}
