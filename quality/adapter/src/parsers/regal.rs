use serde::Deserialize;

use super::{
    as_text, check_output_size, code_name, finding, known_native, FileFinding, ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "regal";

#[derive(Debug, Deserialize)]
struct Report {
    violations: Option<Vec<Violation>>,
    summary: Option<Summary>,
}

#[derive(Debug, Deserialize)]
struct Summary {
    files_scanned: u64,
    num_violations: u64,
}

#[derive(Debug, Deserialize)]
struct Violation {
    title: String,
    description: String,
    category: String,
    level: String,
    location: Option<Location>,
}

#[derive(Debug, Deserialize)]
struct Location {
    file: String,
    row: u64,
    col: u64,
}

/// `regal lint --format json --fail-level warning` exits 0 clean, 2 on warnings only and 3 on any error.
/// Any other exit, non-JSON output, a count mismatch or an exit that disagrees with the levels is an error.
pub fn parse_regal(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if !matches!(code, Some(0 | 2 | 3)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "unexpected exit {}: {} {}",
                code_name(code),
                as_text(TOOL, stderr)?.trim(),
                as_text(TOOL, stdout)?
                    .trim()
                    .chars()
                    .take(600)
                    .collect::<String>()
            ),
        ));
    }
    let report: Report =
        serde_json::from_str(as_text(TOOL, stdout)?).map_err(|source| ParseError::Json {
            tool: TOOL,
            detail: source.to_string(),
        })?;
    let (Some(violations), Some(summary)) = (report.violations, report.summary) else {
        return Err(ParseError::shape(TOOL, "missing violations or summary"));
    };
    if summary.num_violations != violations.len() as u64 {
        return Err(ParseError::shape(
            TOOL,
            "summary.num_violations disagrees with the violation list",
        ));
    }
    if summary.files_scanned > files.len() as u64 {
        return Err(ParseError::shape(
            TOOL,
            "scanned more files than were requested",
        ));
    }
    let mut errors = 0;
    let mut findings = Vec::with_capacity(violations.len());
    for violation in violations {
        let severity = match violation.level.as_str() {
            "error" => {
                errors += 1;
                ToolSeverity::Error
            }
            "warning" => ToolSeverity::Warning,
            other => {
                return Err(ParseError::shape(
                    TOOL,
                    format!("unknown level {other:?} for {}", violation.title),
                ))
            }
        };
        let Some(location) = violation.location else {
            return Err(ParseError::shape(
                TOOL,
                format!("{} without a location", violation.title),
            ));
        };
        let anchored = location.file.is_empty();
        let file = if anchored {
            files.first().copied().ok_or_else(|| {
                ParseError::shape(
                    TOOL,
                    format!("{} without a file or inputs", violation.title),
                )
            })?
        } else {
            known_native(TOOL, files, &location.file)?
        };
        findings.push(finding(
            TOOL,
            file,
            format!("{}/{}", violation.category, violation.title),
            violation.description,
            severity,
            TextPosition {
                line: if anchored { 1 } else { location.row },
                column: if anchored { 1 } else { location.col },
            },
            ColumnEncoding::Scalars,
        ));
    }
    let expected = match (errors, findings.len()) {
        (_, 0) => 0,
        (0, _) => 2,
        _ => 3,
    };
    if code != Some(expected) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} disagrees with {} violations ({errors} errors)",
                code_name(code),
                findings.len()
            ),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["a.rego", "p/b.rego"];

    fn report(level: &str, file: &str, count: u64, scanned: u64) -> String {
        format!(
            r#"{{"violations":[{{"title":"use-assignment-operator","description":"Prefer := for assignment","category":"style","level":"{level}","location":{{"file":"{file}","row":4,"col":2}}}}],"summary":{{"files_scanned":{scanned},"files_failed":1,"rules_skipped":0,"num_violations":{count}}}}}"#
        )
    }

    const CLEAN: &str =
        r#"{"violations":[],"summary":{"files_scanned":2,"files_failed":0,"num_violations":0}}"#;

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_regal(CLEAN.as_bytes(), b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn errors_and_warnings_are_findings() {
        let err = report("error", "p/b.rego", 1, 2);
        let found = parse_regal(err.as_bytes(), b"", Some(3), &FILES).expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "p/b.rego");
        assert_eq!(found[0].finding.rule_id, "style/use-assignment-operator");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.start.column, 2);
        let warn = report("warning", "a.rego", 1, 2);
        let found = parse_regal(warn.as_bytes(), b"", Some(2), &FILES).expect("ok");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn anything_else_is_an_error() {
        let err = report("error", "p/b.rego", 1, 2);
        let warn = report("warning", "p/b.rego", 1, 2);
        assert!(parse_regal(CLEAN.as_bytes(), b"", Some(3), &FILES).is_err());
        assert!(parse_regal(err.as_bytes(), b"", Some(0), &FILES).is_err());
        assert!(parse_regal(err.as_bytes(), b"", Some(2), &FILES).is_err());
        assert!(parse_regal(warn.as_bytes(), b"", Some(3), &FILES).is_err());
        assert!(parse_regal(err.as_bytes(), b"", Some(1), &FILES).is_err());
        assert!(parse_regal(err.as_bytes(), b"", None, &FILES).is_err());
        assert!(parse_regal(b"", b"boom", Some(1), &FILES).is_err());
        assert!(parse_regal(b"not json", b"", Some(0), &FILES).is_err());
        assert!(parse_regal(br#"{"violations":[]}"#, b"", Some(0), &FILES).is_err());
        let count = report("error", "p/b.rego", 2, 2);
        assert!(parse_regal(count.as_bytes(), b"", Some(3), &FILES).is_err());
        let scanned = report("error", "p/b.rego", 1, 3);
        assert!(parse_regal(scanned.as_bytes(), b"", Some(3), &FILES).is_err());
        let stranger = report("error", "other.rego", 1, 2);
        assert!(parse_regal(stranger.as_bytes(), b"", Some(3), &FILES).is_err());
        let level = report("note", "a.rego", 1, 2);
        assert!(parse_regal(level.as_bytes(), b"", Some(3), &FILES).is_err());
    }

    #[test]
    fn a_violation_without_a_file_anchors_to_the_first_input() {
        let aggregate = report("error", "", 1, 2);
        let found = parse_regal(aggregate.as_bytes(), b"", Some(3), &FILES).expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "a.rego");
        assert!(parse_regal(aggregate.as_bytes(), b"", Some(3), &[]).is_err());
    }

    #[test]
    fn ignored_files_may_scan_fewer() {
        let partial = CLEAN.replace("\"files_scanned\":2", "\"files_scanned\":1");
        assert!(parse_regal(partial.as_bytes(), b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }
}
