use super::{as_text, code_name, known_spelled, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "trivy_config";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

/// Parses `trivy config --format json --exit-code 0`: one report whose `Results[].Misconfigurations[]`
/// are the findings. CRITICAL and HIGH are errors, MEDIUM warnings, LOW and UNKNOWN info.
pub fn parse_trivy_config(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stderr).trim()
            ),
        ));
    }
    let report: serde_json::Value =
        serde_json::from_str(as_text(TOOL, stdout)?.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let results = match report.get("Results") {
        None | Some(serde_json::Value::Null) => return Ok(Vec::new()),
        Some(results) => results
            .as_array()
            .ok_or_else(|| ParseError::shape(TOOL, "Results is not an array"))?,
    };
    let mut findings = Vec::new();
    for result in results {
        let target = text(result, "Target")?;
        let misconfigurations = match result.get("Misconfigurations") {
            None | Some(serde_json::Value::Null) => continue,
            Some(list) => list
                .as_array()
                .ok_or_else(|| ParseError::shape(TOOL, "Misconfigurations is not an array"))?,
        };
        let file = known_spelled(TOOL, files, target)?;
        for entry in misconfigurations {
            let severity = match text(entry, "Severity")? {
                "CRITICAL" | "HIGH" => ToolSeverity::Error,
                "MEDIUM" => ToolSeverity::Warning,
                "LOW" | "UNKNOWN" => ToolSeverity::Info,
                other => {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("unknown severity {other:?}"),
                    ))
                }
            };
            let cause = entry.get("CauseMetadata");
            let line = |key: &str| {
                cause
                    .and_then(|cause| cause.get(key))
                    .and_then(serde_json::Value::as_u64)
                    .filter(|line| *line > 0)
            };
            let first = line("StartLine").unwrap_or(1);
            let (start, _) = point(first, 1);
            let end = line("EndLine")
                .filter(|last| *last > first)
                .map(|last| TextPosition {
                    line: last,
                    column: 1,
                });
            findings.push(FileFinding {
                file: file.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: text(entry, "ID")?.to_owned(),
                    message: text(entry, "Title")?.trim().to_owned(),
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"SchemaVersion":2,"Results":[{"Target":"Dockerfile","Class":"config","Type":"dockerfile","MisconfSummary":{"Successes":0,"Failures":2},"Misconfigurations":[{"ID":"DS-0002","Title":"Image user should not be 'root'","Severity":"HIGH","CauseMetadata":{"StartLine":1,"EndLine":3}},{"ID":"DS-0026","Title":"No HEALTHCHECK defined","Severity":"LOW","CauseMetadata":{}}]}]}"#;

    #[test]
    fn a_clean_run_has_no_results() {
        let clean = parse_trivy_config(b"{\"SchemaVersion\":2}", b"", Some(0), &["Dockerfile"])
            .expect("parsed");
        assert!(clean.is_empty());
        let passing = br#"{"Results":[{"Target":"Dockerfile","Misconfigurations":null}]}"#;
        assert!(parse_trivy_config(passing, b"", Some(0), &["Dockerfile"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn misconfigurations_become_findings_with_mapped_severity() {
        let findings =
            parse_trivy_config(DIRTY.as_bytes(), b"", Some(0), &["Dockerfile"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Dockerfile");
        assert_eq!(findings[0].finding.tool_id, "trivy_config");
        assert_eq!(findings[0].finding.rule_id, "DS-0002");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.end.map(|end| end.line), Some(3));
        assert_eq!(findings[1].finding.severity, ToolSeverity::Info);
        assert_eq!(findings[1].finding.start.line, 1);
        assert!(findings[1].finding.end.is_none());
    }

    #[test]
    fn medium_is_a_warning() {
        let medium = DIRTY.replace("HIGH", "MEDIUM");
        let findings =
            parse_trivy_config(medium.as_bytes(), b"", Some(0), &["Dockerfile"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn unknown_files_severities_and_exits_are_errors() {
        assert!(parse_trivy_config(DIRTY.as_bytes(), b"", Some(0), &["other"]).is_err());
        let odd = DIRTY.replace("HIGH", "SEVERE");
        assert!(parse_trivy_config(odd.as_bytes(), b"", Some(0), &["Dockerfile"]).is_err());
        assert!(parse_trivy_config(b"", b"boom", Some(1), &["Dockerfile"]).is_err());
        assert!(parse_trivy_config(b"not json", b"", Some(0), &["Dockerfile"]).is_err());
    }
}
