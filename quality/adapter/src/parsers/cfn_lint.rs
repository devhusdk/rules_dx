use super::{as_text, code_name, known_spelled, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "cfn_lint";

const EXIT_ERROR: i32 = 2;
const EXIT_WARNING: i32 = 4;
const EXIT_INFO: i32 = 8;

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn place(value: &serde_json::Value, key: &str) -> Result<(u64, u64), ParseError> {
    let at = value
        .get("Location")
        .and_then(|location| location.get(key))
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing Location.{key}")))?;
    let number = |name: &str| {
        at.get(name)
            .and_then(serde_json::Value::as_u64)
            .map(|number| number.max(1))
            .ok_or_else(|| ParseError::shape(TOOL, format!("missing Location.{key}.{name}")))
    };
    Ok((number("LineNumber")?, number("ColumnNumber")?))
}

/// Parses `cfn-lint --format json --non-zero-exit-code informational`: the exit code is a bitmask of the levels reported.
pub fn parse_cfn_lint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let Some(exit) =
        code.filter(|exit| *exit >= 0 && exit & !(EXIT_ERROR | EXIT_WARNING | EXIT_INFO) == 0)
    else {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stdout)
                    .lines()
                    .chain(String::from_utf8_lossy(stderr).lines())
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("no output")
                    .trim()
            ),
        });
    };
    let report = as_text(TOOL, stdout)?;
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(report.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut seen = 0;
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        let filename = entry
            .get("Filename")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ParseError::shape(
                    TOOL,
                    format!(
                        "finding without a file: {}",
                        text(entry, "Message").unwrap_or("")
                    ),
                )
            })?;
        let checked = known_spelled(TOOL, files, filename)?;
        let (severity, bit) = match text(entry, "Level")? {
            "Error" => (ToolSeverity::Error, EXIT_ERROR),
            "Warning" => (ToolSeverity::Warning, EXIT_WARNING),
            "Informational" => (ToolSeverity::Info, EXIT_INFO),
            other => return Err(ParseError::shape(TOOL, format!("unknown level {other:?}"))),
        };
        seen |= bit;
        let rule_id = entry
            .get("Rule")
            .and_then(|rule| rule.get("Id"))
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| ParseError::shape(TOOL, "missing Rule.Id"))?;
        let message = text(entry, "Message")?.trim();
        if message.is_empty() {
            return Err(ParseError::shape(TOOL, "missing Message"));
        }
        let (line, column) = place(entry, "Start")?;
        let (start, _) = point(line, column);
        let end = place(entry, "End")
            .ok()
            .map(|(line, column)| crate::TextPosition { line, column });
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule_id.to_owned(),
                message: message.to_owned(),
                severity,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if seen != exit {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {exit} does not match the {} reported finding(s)",
                findings.len()
            ),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded from cfn-lint 1.58.0 with the argv `cfn_lint_check` builds, files named `/scratch/<name>.yaml`, whitespace removed.
    const DIRTY_ERROR: &str = r#"[{"Filename":"/scratch/err.yaml","Id":"7020632d-c87f-0e1b-508e-f56cf783d086","Level":"Error","Location":{"End":{"ColumnNumber":12,"LineNumber":7},"Path":["Resources","Bucket","Properties","Bogus"],"Start":{"ColumnNumber":7,"LineNumber":7}},"Message":"Additional properties are not allowed ('Bogus' was unexpected)","ParentId":null,"Rule":{"Description":"Making sure that resources properties are properly configured","Id":"E3002","ShortDescription":"Resource properties are invalid","Source":"https://github.com/aws-cloudformation/cfn-lint/blob/main/docs/cfn-schema-specification.md#properties"}}]"#;
    const DIRTY_WARNING: &str = r#"[{"Filename":"/scratch/warn.yaml","Id":"d65377d7-c0bb-ccaa-e2e6-3e2b33db0252","Level":"Warning","Location":{"End":{"ColumnNumber":9,"LineNumber":3},"Path":["Parameters","Unused"],"Start":{"ColumnNumber":3,"LineNumber":3}},"Message":"Parameter Unused not used.","ParentId":null,"Rule":{"Description":"Making sure the parameters defined are used","Id":"W2001","ShortDescription":"Check if Parameters are Used","Source":"https://github.com/aws-cloudformation/cfn-lint"}},{"Filename":"/scratch/warn.yaml","Id":"76492e06-c810-33d9-f1c2-8344c81c2c91","Level":"Warning","Location":{"End":{"ColumnNumber":16,"LineNumber":9},"Path":["Resources","Topic","Properties","TopicName","Fn::Sub"],"Start":{"ColumnNumber":7,"LineNumber":9}},"Message":"'Fn::Sub' isn't needed because there are no variables","ParentId":null,"Rule":{"Description":"Checks sub strings to see if a variable is defined.","Id":"W1020","ShortDescription":"Sub isn't needed if it doesn't have a variable defined","Source":"https://docs.aws.amazon.com/AWSCloudFormation/latest/UserGuide/intrinsic-function-reference-sub.html"}}]"#;
    const SYNTAX_ERROR: &str = r#"[{"Filename":"/scratch/syn.yaml","Id":"9cd63f65-268c-392a-1b81-1efadd15c256","Level":"Error","Location":{"End":{"ColumnNumber":2,"LineNumber":3},"Path":null,"Start":{"ColumnNumber":1,"LineNumber":3}},"Message":"did not find expected node content","ParentId":null,"Rule":{"Description":"Checks for JSON/YAML formatting errors in your template","Id":"E0000","ShortDescription":"Parsing error found when parsing the template","Source":"https://github.com/aws-cloudformation/cfn-lint"}}]"#;
    const UNICODE_ERROR: &str = r#"[{"Filename":"/scratch/uni.yaml","Id":"23584b27-be70-3723-528b-937186cd238c","Level":"Error","Location":{"End":{"ColumnNumber":19,"LineNumber":6},"Path":null,"Start":{"ColumnNumber":18,"LineNumber":6}},"Message":"did not find expected key","ParentId":null,"Rule":{"Description":"Checks for JSON/YAML formatting errors in your template","Id":"E0000","ShortDescription":"Parsing error found when parsing the template","Source":"https://github.com/aws-cloudformation/cfn-lint"}}]"#;
    const MISSING_FILE: &str = r#"[{"Filename":null,"Id":"84959da2-609c-004d-c566-20375c7d0ef0","Level":"Error","Location":{"End":{"ColumnNumber":1,"LineNumber":1},"Path":null,"Start":{"ColumnNumber":1,"LineNumber":1}},"Message":"missing.yaml could not be processed by glob.glob","ParentId":null,"Rule":{"Description":"Error as a result of the cfn-lint configuration","Id":"E0003","ShortDescription":"Error with cfn-lint configuration","Source":"https://github.com/aws-cloudformation/cfn-lint"}}]"#;

    fn files() -> [&'static str; 5] {
        [
            "/scratch/err.yaml",
            "/scratch/warn.yaml",
            "/scratch/syn.yaml",
            "/scratch/uni.yaml",
            "/scratch/clean.yaml",
        ]
    }

    #[test]
    fn clean_run_is_clean() {
        assert!(parse_cfn_lint(b"[]\n", b"", Some(0), &files())
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn error_finding_carries_rule_and_range() {
        let out = parse_cfn_lint(DIRTY_ERROR.as_bytes(), b"", Some(2), &files()).expect("dirty");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file, "/scratch/err.yaml");
        assert_eq!(out[0].finding.rule_id, "E3002");
        assert_eq!(out[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (out[0].finding.start.line, out[0].finding.start.column),
            (7, 7)
        );
        let end = out[0].finding.end.expect("end");
        assert_eq!((end.line, end.column), (7, 12));
    }

    #[test]
    fn warnings_use_exit_four() {
        let out = parse_cfn_lint(DIRTY_WARNING.as_bytes(), b"", Some(4), &files()).expect("warn");
        assert_eq!(out.len(), 2);
        assert!(out
            .iter()
            .all(|item| item.finding.severity == ToolSeverity::Warning));
        assert!(parse_cfn_lint(DIRTY_WARNING.as_bytes(), b"", Some(2), &files()).is_err());
        assert!(parse_cfn_lint(DIRTY_WARNING.as_bytes(), b"", Some(0), &files()).is_err());
    }

    #[test]
    fn informational_maps_to_info() {
        let body = DIRTY_WARNING.replace("\"Warning\"", "\"Informational\"");
        let out = parse_cfn_lint(body.as_bytes(), b"", Some(8), &files()).expect("info");
        assert_eq!(out[0].finding.severity, ToolSeverity::Info);
        assert!(parse_cfn_lint(body.as_bytes(), b"", Some(4), &files()).is_err());
    }

    #[test]
    fn mixed_levels_need_the_union_of_bits() {
        let both = format!(
            "[{},{}]",
            &DIRTY_ERROR[1..DIRTY_ERROR.len() - 1],
            &DIRTY_WARNING[1..DIRTY_WARNING.len() - 1]
        );
        assert!(parse_cfn_lint(both.as_bytes(), b"", Some(6), &files()).is_ok());
        assert!(parse_cfn_lint(both.as_bytes(), b"", Some(2), &files()).is_err());
    }

    #[test]
    fn syntax_error_is_a_located_error() {
        let out = parse_cfn_lint(SYNTAX_ERROR.as_bytes(), b"", Some(2), &files()).expect("syntax");
        assert_eq!(out[0].finding.rule_id, "E0000");
        assert_eq!(out[0].finding.start.line, 3);
    }

    #[test]
    fn columns_count_unicode_scalars() {
        let out = parse_cfn_lint(UNICODE_ERROR.as_bytes(), b"", Some(2), &files()).expect("uni");
        assert_eq!(out[0].finding.start.column, 18);
        assert_eq!(out[0].finding.encoding, ColumnEncoding::Scalars);
    }

    #[test]
    fn finding_without_a_file_is_not_clean() {
        let err = parse_cfn_lint(MISSING_FILE.as_bytes(), b"", Some(2), &files())
            .expect_err("null filename fails");
        assert!(matches!(err, ParseError::Shape { .. }));
    }

    #[test]
    fn unknown_file_is_rejected() {
        let body = DIRTY_ERROR.replace("/scratch/err.yaml", "/elsewhere/x.yaml");
        let err = parse_cfn_lint(body.as_bytes(), b"", Some(2), &files()).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }

    #[test]
    fn disagreeing_exit_and_output_never_pass_as_clean() {
        assert!(parse_cfn_lint(b"[]", b"", Some(2), &files()).is_err());
        assert!(parse_cfn_lint(DIRTY_ERROR.as_bytes(), b"", Some(0), &files()).is_err());
        assert!(parse_cfn_lint(b"[]", b"", Some(1), &files()).is_err());
        assert!(parse_cfn_lint(b"[]", b"", Some(3), &files()).is_err());
        assert!(parse_cfn_lint(b"[]", b"", Some(16), &files()).is_err());
        assert!(parse_cfn_lint(b"[]", b"", None, &files()).is_err());
    }

    #[test]
    fn exit_one_reports_the_config_message() {
        let err = parse_cfn_lint(
            b"Invalid configuration key 'bogus_key' in .cfnlintrc at bogus_key\n",
            b"",
            Some(1),
            &files(),
        )
        .expect_err("exit 1");
        assert!(err.to_string().contains("Invalid configuration key"));
    }

    #[test]
    fn malformed_output_fails() {
        assert!(parse_cfn_lint(b"not json", b"", Some(0), &files()).is_err());
        assert!(parse_cfn_lint(b"{}", b"", Some(0), &files()).is_err());
        assert!(parse_cfn_lint(&[0xff, 0xfe], b"", Some(0), &files()).is_err());
        assert!(parse_cfn_lint(b"[{}]", b"", Some(2), &files()).is_err());
    }
}
