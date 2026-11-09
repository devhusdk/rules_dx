use super::{FileFinding, ParseError, as_text, code_name, point};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "container_structure_test";

/// Parses `container-structure-test test --output json`: each failed entry of `Results` is one
/// finding per error, on `file` line 1, naming the image `name`. The rule is the test kind before
/// the colon of the entry name, such as `File Existence Test`.
pub fn parse_container_structure_test(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    name: &str,
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
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
    let results = report
        .get("Results")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing Results"))?;
    let (start, _) = point(1, 1);
    let mut findings = Vec::new();
    for result in results {
        let test = result
            .get("Name")
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| ParseError::shape(TOOL, "missing Name"))?;
        let passed = result
            .get("Pass")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| ParseError::shape(TOOL, "missing Pass"))?;
        if passed {
            continue;
        }
        let errors: Vec<&str> = match result.get("Errors") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(list) => list
                .as_array()
                .ok_or_else(|| ParseError::shape(TOOL, "Errors is not an array"))?
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect(),
        };
        let rule = test.split(':').next().unwrap_or(test).trim();
        let messages = if errors.is_empty() {
            vec!["failed"]
        } else {
            errors
        };
        for message in messages {
            findings.push(FileFinding {
                file: file.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: rule.to_owned(),
                    message: format!("{name}: {test}: {}", message.trim()),
                    severity: ToolSeverity::Error,
                    start,
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
        }
    }
    if code == Some(1) && findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 1 without a failed test".to_owned(),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAILED: &str = r#"{"Pass":1,"Fail":2,"Total":3,"Duration":0,"Results":[{"Name":"File Existence Test: bin","Pass":false,"Errors":["no such file"],"Duration":0},{"Name":"Metadata Test","Pass":false,"Errors":["entrypoint differs","user differs"],"Duration":0},{"Name":"License Test","Pass":true,"Duration":0}]}"#;

    fn parse(stdout: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_container_structure_test(
            stdout.as_bytes(),
            b"",
            Some(code),
            "image",
            "BUILD.bazel",
        )
    }

    #[test]
    fn a_passing_run_has_no_findings() {
        let passing = r#"{"Pass":1,"Fail":0,"Total":1,"Results":[{"Name":"Metadata Test","Pass":true}]}"#;
        assert!(parse(passing, 0).expect("parsed").is_empty());
    }

    #[test]
    fn failed_tests_land_on_the_build_file_and_name_the_image() {
        let findings = parse(FAILED, 1).expect("parsed");
        assert_eq!(findings.len(), 3);
        assert!(findings.iter().all(|found| found.file == "BUILD.bazel"));
        assert!(
            findings
                .iter()
                .all(|found| found.finding.tool_id == "container_structure_test")
        );
        assert_eq!(findings[0].finding.rule_id, "File Existence Test");
        assert_eq!(
            findings[0].finding.message,
            "image: File Existence Test: bin: no such file"
        );
        assert_eq!(findings[1].finding.rule_id, "Metadata Test");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
    }

    #[test]
    fn exits_and_shapes_outside_the_grammar_are_errors() {
        assert!(parse(FAILED, 2).is_err());
        assert!(parse("not json", 1).is_err());
        assert!(parse("{}", 0).is_err());
        let empty = r#"{"Results":[]}"#;
        assert!(parse(empty, 1).is_err());
    }
}
