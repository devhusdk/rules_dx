use super::{FileFinding, ParseError, as_text, code_name, point};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "trivy_image_config";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn severity(entry: &serde_json::Value) -> Result<ToolSeverity, ParseError> {
    match text(entry, "Severity")? {
        "CRITICAL" | "HIGH" => Ok(ToolSeverity::Error),
        "MEDIUM" => Ok(ToolSeverity::Warning),
        "LOW" | "UNKNOWN" => Ok(ToolSeverity::Info),
        other => Err(ParseError::shape(
            TOOL,
            format!("unknown severity {other:?}"),
        )),
    }
}

fn entries<'a>(
    result: &'a serde_json::Value,
    key: &str,
) -> Result<&'a [serde_json::Value], ParseError> {
    match result.get(key) {
        None | Some(serde_json::Value::Null) => Ok(&[]),
        Some(list) => list
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| ParseError::shape(TOOL, format!("{key} is not an array"))),
    }
}

/// Parses `trivy image --input <layout> --format json --exit-code 0`: `Results[].Misconfigurations[]` and
/// `Results[].Secrets[]` are the findings. Every finding lands on `file` line 1 and names the image
/// `name`; a secret found in a layer file also names that file. CRITICAL and HIGH are errors, MEDIUM
/// warnings, LOW and UNKNOWN info.
pub fn parse_trivy_image_config(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &str,
    name: &str,
    file: &str,
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
        let place = if target == input {
            String::new()
        } else {
            format!(" in {target}")
        };
        let (start, _) = point(1, 1);
        let mut push = |entry: &serde_json::Value, rule: &str| -> Result<(), ParseError> {
            findings.push(FileFinding {
                file: file.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: text(entry, rule)?.to_owned(),
                    message: format!("{name}: {}{place}", text(entry, "Title")?.trim()),
                    severity: severity(entry)?,
                    start,
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
            Ok(())
        };
        for entry in entries(result, "Misconfigurations")? {
            push(entry, "ID")?;
        }
        for entry in entries(result, "Secrets")? {
            push(entry, "RuleID")?;
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"SchemaVersion":2,"Results":[{"Target":"/out/image","Class":"config","Type":"dockerfile","Misconfigurations":[{"ID":"DS-0002","Title":"Image user should not be 'root'","Severity":"HIGH"},{"ID":"DS-0026","Title":"No HEALTHCHECK defined","Severity":"LOW"}]},{"Target":"/out/image","Class":"secret","Secrets":[{"RuleID":"aws-access-key-id","Title":"AWS Access Key ID","Severity":"CRITICAL","StartLine":28}]},{"Target":"etc/key.pem","Class":"secret","Secrets":[{"RuleID":"private-key","Title":"Asymmetric Private Key","Severity":"HIGH"}]}]}"#;

    fn parse(stdout: &str) -> Result<Vec<FileFinding>, ParseError> {
        parse_trivy_image_config(
            stdout.as_bytes(),
            b"",
            Some(0),
            "/out/image",
            "image",
            "BUILD.bazel",
        )
    }

    #[test]
    fn a_clean_run_has_no_results() {
        assert!(parse("{\"SchemaVersion\":2}").expect("parsed").is_empty());
        let passing = r#"{"Results":[{"Target":"/out/image","Misconfigurations":null}]}"#;
        assert!(parse(passing).expect("parsed").is_empty());
    }

    #[test]
    fn findings_land_on_the_build_file_and_name_the_image() {
        let findings = parse(DIRTY).expect("parsed");
        assert_eq!(findings.len(), 4);
        assert!(findings.iter().all(|found| found.file == "BUILD.bazel"));
        assert!(
            findings
                .iter()
                .all(|found| found.finding.tool_id == "trivy_image_config")
        );
        assert_eq!(findings[0].finding.rule_id, "DS-0002");
        assert_eq!(
            findings[0].finding.message,
            "image: Image user should not be 'root'"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Info);
        assert_eq!(findings[2].finding.rule_id, "aws-access-key-id");
        assert_eq!(findings[2].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[2].finding.start.line, 1);
        assert_eq!(
            findings[3].finding.message,
            "image: Asymmetric Private Key in etc/key.pem"
        );
    }

    #[test]
    fn medium_is_a_warning() {
        let medium = DIRTY.replace("HIGH", "MEDIUM");
        assert_eq!(
            parse(&medium).expect("parsed")[0].finding.severity,
            ToolSeverity::Warning
        );
    }

    #[test]
    fn unknown_severities_and_exits_are_errors() {
        assert!(parse(&DIRTY.replace("HIGH", "SEVERE")).is_err());
        assert!(
            parse_trivy_image_config(b"", b"boom", Some(1), "/out/image", "image", "BUILD.bazel")
                .is_err()
        );
        assert!(parse("not json").is_err());
    }
}
