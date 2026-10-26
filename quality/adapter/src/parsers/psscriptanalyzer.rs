use super::{check_output_size, code_name, known, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "psscriptanalyzer";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn position(value: &serde_json::Value, key: &str) -> Result<u64, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|number| number.max(1))
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

pub fn parse_psscriptanalyzer(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::shape(TOOL, format!("exit {}", code_name(code))));
    }
    let report: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    if let Some(error) = report.get("error").and_then(serde_json::Value::as_str) {
        return Err(ParseError::shape(TOOL, error));
    }
    let entries = report
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing files"))?;
    let mut findings = Vec::new();
    let mut reported: Vec<&str> = Vec::new();
    for entry in entries {
        let checked = known(TOOL, files, text(entry, "path")?)?;
        reported.push(checked);
        let diagnostics = entry
            .get("diagnostics")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ParseError::shape(TOOL, "missing diagnostics"))?;
        for diagnostic in diagnostics {
            let severity = match text(diagnostic, "severity")? {
                "Information" => ToolSeverity::Info,
                "Warning" => ToolSeverity::Warning,
                "Error" | "ParseError" => ToolSeverity::Error,
                other => {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("unknown severity {other:?}"),
                    ))
                }
            };
            let message = text(diagnostic, "message")?;
            if message.is_empty() {
                return Err(ParseError::shape(TOOL, "missing message"));
            }
            let (start, end) = point(
                position(diagnostic, "line")?,
                position(diagnostic, "column")?,
            );
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: text(diagnostic, "rule")?.to_owned(),
                    message: message.to_owned(),
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf16,
                    related: Vec::new(),
                },
            });
        }
    }
    if let Some(missing) = files.iter().find(|file| !reported.contains(file)) {
        return Err(ParseError::shape(TOOL, format!("no result for {missing}")));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = r#"{"analyzer":"1.25.0","files":[{"path":"Sample.ps1","diagnostics":[{"rule":"PSAvoidUsingWriteHost","severity":"Warning","line":4,"column":1,"message":"Avoid using Write-Host"}]}]}"#;
    const CLEAN: &str = r#"{"analyzer":"1.25.0","files":[{"path":"Sample.ps1","diagnostics":[]}]}"#;

    #[test]
    fn psscriptanalyzer_reports_rule_severity_and_position() {
        let findings =
            parse_psscriptanalyzer(DIRTY.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "PSAvoidUsingWriteHost");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.line, 4);
        let clean =
            parse_psscriptanalyzer(CLEAN.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert!(clean.is_empty());
    }

    #[test]
    fn parse_errors_are_errors() {
        let broken = DIRTY.replace("Warning", "ParseError");
        let findings =
            parse_psscriptanalyzer(broken.as_bytes(), Some(0), &["Sample.ps1"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn psscriptanalyzer_never_reports_a_failed_run_as_clean() {
        assert!(parse_psscriptanalyzer(b"", Some(0), &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(CLEAN.as_bytes(), Some(1), &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(CLEAN.as_bytes(), None, &["Sample.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(DIRTY.as_bytes(), Some(0), &["other.ps1"]).is_err());
        assert!(parse_psscriptanalyzer(&[0xff], Some(0), &["x"]).is_err());
        assert!(
            parse_psscriptanalyzer(CLEAN.as_bytes(), Some(0), &["Sample.ps1", "Other.ps1"])
                .is_err()
        );
        let failed = r#"{"error":"IncludeRules names the unknown rule Nope"}"#;
        let error = parse_psscriptanalyzer(failed.as_bytes(), Some(0), &["Sample.ps1"])
            .expect_err("failed");
        assert!(error.to_string().contains("unknown rule Nope"), "{error}");
        let odd = DIRTY.replace("Warning", "Fatal");
        assert!(parse_psscriptanalyzer(odd.as_bytes(), Some(0), &["Sample.ps1"]).is_err());
    }
}
