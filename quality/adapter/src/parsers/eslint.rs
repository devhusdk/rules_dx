use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct EslintFile {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<EslintMessage>,
}

#[derive(Debug, Deserialize)]
struct EslintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    fatal: Option<bool>,
    severity: u64,
    message: String,
    #[serde(default)]
    line: u64,
    #[serde(default)]
    column: u64,
    #[serde(rename = "endLine")]
    end_line: Option<u64>,
    #[serde(rename = "endColumn")]
    end_column: Option<u64>,
}

fn eslint_severity(tool: &'static str, severity: u64) -> Result<ToolSeverity, ParseError> {
    match severity {
        2 => Ok(ToolSeverity::Error),
        1 => Ok(ToolSeverity::Warning),
        _ => Err(ParseError::Shape {
            tool,
            detail: format!("unknown severity: {severity}"),
        }),
    }
}

pub fn parse_eslint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    parse_eslint_json("eslint", false, stdout, code, files)
}

/// Parses graphql-eslint JSON, which must report every requested file.
pub fn parse_graphql_eslint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    parse_eslint_json("graphql_eslint", true, stdout, code, files)
}

fn parse_eslint_json(
    tool: &'static str,
    require_every_file: bool,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stdout)?;
    let reports: Vec<EslintFile> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool,
            detail: err.to_string(),
        })?;
    if require_every_file {
        for file in files {
            if !reports.iter().any(|report| report.file_path == *file) {
                return Err(ParseError::Shape {
                    tool,
                    detail: format!("no result for {file}"),
                });
            }
        }
    }
    let mut findings = Vec::new();
    for report in &reports {
        let checked = known(tool, files, &report.file_path)?;
        for message in &report.messages {
            if message.message.is_empty() {
                return Err(ParseError::Shape {
                    tool,
                    detail: "message with empty text".to_owned(),
                });
            }
            let rule_id = match (&message.rule_id, message.fatal) {
                (Some(rule), _) if !rule.is_empty() => rule.clone(),
                (Some(_), _) | (None, _) if message.fatal == Some(true) => String::new(),
                _ => {
                    return Err(ParseError::Shape {
                        tool,
                        detail: format!("ignored file {:?}", report.file_path),
                    });
                }
            };
            if message.line < 1 || message.column < 1 {
                return Err(ParseError::Shape {
                    tool,
                    detail: format!("bad position {}:{}", message.line, message.column),
                });
            }
            let start = TextPosition {
                line: message.line,
                column: message.column,
            };
            let end = match (message.end_line, message.end_column) {
                (Some(line), Some(column)) => {
                    if line < 1 || column < 1 {
                        return Err(ParseError::Shape {
                            tool,
                            detail: "bad end position".to_owned(),
                        });
                    }
                    Some(TextPosition { line, column })
                }
                (None, None) => None,
                _ => {
                    return Err(ParseError::Shape {
                        tool,
                        detail: "partial end position".to_owned(),
                    });
                }
            };
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: tool.to_owned(),
                    rule_id,
                    message: message.message.clone(),
                    severity: eslint_severity(tool, message.severity)?,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf16,
                    related: Vec::new(),
                },
            });
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no messages", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESLINT_DIRTY: &str = r#"[{"filePath":"/s/dirty.js","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"'unusedVar' is assigned a value but never used.","line":1,"column":7,"endLine":1,"endColumn":16}],"errorCount":1,"warningCount":0}]"#;

    const ESLINT_CLEAN: &str =
        r#"[{"filePath":"/s/clean.js","messages":[],"errorCount":0,"warningCount":0}]"#;

    #[test]
    fn eslint_reports_rules_with_extents() {
        let findings =
            parse_eslint(ESLINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "no-unused-vars");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        let end = findings[0].finding.end.expect("extent");
        assert_eq!((end.line, end.column), (1, 16));
        let clean =
            parse_eslint(ESLINT_CLEAN.as_bytes(), Some(0), &["/s/clean.js"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_eslint(ESLINT_CLEAN.as_bytes(), Some(1), &["/s/clean.js"]).is_err());
        let fatal_null = r#"[{"filePath":"/s/broken.js","messages":[{"ruleId":null,"fatal":true,"severity":2,"message":"Parsing error: Unexpected token","line":2,"column":1}],"errorCount":1}]"#;
        let findings =
            parse_eslint(fatal_null.as_bytes(), Some(1), &["/s/broken.js"]).expect("parsed");
        assert_eq!(findings[0].finding.rule_id, "");
        let ignored = r#"[{"filePath":"/s/a.js","messages":[{"ruleId":null,"fatal":false,"severity":1,"message":"File ignored because outside of base path.","line":1,"column":1}],"warningCount":1}]"#;
        let err = parse_eslint(ignored.as_bytes(), Some(0), &["/s/a.js"]).expect_err("ignored");
        assert!(err.to_string().contains("ignored file"));
        let unmatched = r#"[{"filePath":"/s/App.vue","messages":[{"ruleId":null,"fatal":false,"severity":1,"message":"File ignored because no matching configuration was supplied."}],"warningCount":1}]"#;
        let err =
            parse_eslint(unmatched.as_bytes(), Some(0), &["/s/App.vue"]).expect_err("unmatched");
        assert!(err.to_string().contains("ignored file"));
        let bad_sev = ESLINT_DIRTY.replace("\"severity\":2", "\"severity\":3");
        assert!(parse_eslint(bad_sev.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let warning = ESLINT_DIRTY.replace("\"severity\":2", "\"severity\":1");
        let findings = parse_eslint(warning.as_bytes(), Some(1), &["/s/dirty.js"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let empty_message = ESLINT_DIRTY.replace(
            "\"message\":\"'unusedVar' is assigned a value but never used.\"",
            "\"message\":\"\"",
        );
        assert!(parse_eslint(empty_message.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let bad_pos = ESLINT_DIRTY.replace("\"line\":1,\"column\":7", "\"line\":0,\"column\":7");
        assert!(parse_eslint(bad_pos.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let bad_end = ESLINT_DIRTY.replace(
            "\"endLine\":1,\"endColumn\":16",
            "\"endLine\":0,\"endColumn\":16",
        );
        assert!(parse_eslint(bad_end.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let partial_end = ESLINT_DIRTY.replace(",\"endColumn\":16", "");
        assert!(parse_eslint(partial_end.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        assert!(parse_eslint(b"not json", Some(1), &["/s/dirty.js"]).is_err());
    }

    #[test]
    fn eslint_columns_are_utf16_after_astral_characters() {
        let source = "const s = \"\u{1f600}\"; missing();\n";
        let report = r#"[{"filePath":"/s/a.js","messages":[{"ruleId":"no-undef","severity":2,"message":"'missing' is not defined.","line":1,"column":17,"endLine":1,"endColumn":24}]}]"#;
        let findings = parse_eslint(report.as_bytes(), Some(1), &["/s/a.js"]).expect("parsed");
        let placed = crate::place_finding(&findings[0].finding, "a.js", source).expect("placed");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(18), Some(25)));
        let at_end = r#"[{"filePath":"/s/a.js","messages":[{"ruleId":"no-undef","severity":2,"message":"m","line":1,"column":17,"endLine":1,"endColumn":27}]}]"#;
        let findings = parse_eslint(at_end.as_bytes(), Some(1), &["/s/a.js"]).expect("parsed");
        let placed = crate::place_finding(&findings[0].finding, "a.js", source).expect("placed");
        assert_eq!(placed.end_byte, Some(28));
    }

    const GRAPHQL_DIRTY: &str = r#"[{"filePath":"/s/api/query.graphql","messages":[{"ruleId":"@graphql-eslint/fields-on-correct-type","severity":2,"message":"Cannot query field \"email\" on type \"User\".","line":3,"column":5,"endLine":3,"endColumn":10}],"errorCount":1,"warningCount":0}]"#;

    const GRAPHQL_CLEAN: &str =
        r#"[{"filePath":"/s/api/query.graphql","messages":[],"errorCount":0,"warningCount":0}]"#;

    #[test]
    fn graphql_eslint_reports_clean_and_dirty_files() {
        let files = ["/s/api/query.graphql"];
        let clean = parse_graphql_eslint(GRAPHQL_CLEAN.as_bytes(), Some(0), &files).expect("clean");
        assert!(clean.is_empty());
        let findings =
            parse_graphql_eslint(GRAPHQL_DIRTY.as_bytes(), Some(1), &files).expect("dirty");
        assert_eq!(findings.len(), 1);
        let found = &findings[0];
        assert_eq!(found.file, "/s/api/query.graphql");
        assert_eq!(found.finding.tool_id, "graphql_eslint");
        assert_eq!(
            found.finding.rule_id,
            "@graphql-eslint/fields-on-correct-type"
        );
        assert_eq!(
            (found.finding.start.line, found.finding.start.column),
            (3, 5)
        );
        assert_eq!(found.finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn graphql_eslint_reports_fatal_syntax_errors() {
        let fatal = r#"[{"filePath":"/s/api/bad.graphql","messages":[{"ruleId":null,"fatal":true,"severity":2,"message":"Parsing error: [graphql-eslint] Syntax Error: Expected Name, found \"{\".","line":2,"column":15}],"errorCount":1}]"#;
        let findings = parse_graphql_eslint(fatal.as_bytes(), Some(1), &["/s/api/bad.graphql"])
            .expect("fatal");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "");
        assert_eq!(findings[0].finding.tool_id, "graphql_eslint");
    }

    #[test]
    fn graphql_eslint_failures_are_errors_not_clean() {
        let files = ["/s/api/query.graphql", "/s/api/other.graphql"];
        let err = parse_graphql_eslint(GRAPHQL_CLEAN.as_bytes(), Some(0), &files)
            .expect_err("missing file");
        assert!(err
            .to_string()
            .contains("no result for /s/api/other.graphql"));
        assert!(parse_graphql_eslint(b"not json", Some(1), &files[..1]).is_err());
        assert!(parse_graphql_eslint(b"", Some(2), &files[..1]).is_err());
        assert!(parse_graphql_eslint(GRAPHQL_CLEAN.as_bytes(), Some(1), &files[..1]).is_err());
        let ignored = r#"[{"filePath":"/s/api/query.graphql","messages":[{"ruleId":null,"fatal":false,"severity":1,"message":"File ignored because no matching configuration was supplied.","line":1,"column":1}],"warningCount":1}]"#;
        assert!(parse_graphql_eslint(ignored.as_bytes(), Some(0), &files[..1]).is_err());
        let stranger = GRAPHQL_DIRTY.replace("/s/api/query.graphql", "/s/elsewhere.graphql");
        assert!(parse_graphql_eslint(stranger.as_bytes(), Some(1), &files[..1]).is_err());
    }

    #[test]
    fn eslint_still_tolerates_files_missing_from_the_report() {
        let findings = parse_eslint(
            ESLINT_CLEAN.as_bytes(),
            Some(0),
            &["/s/clean.js", "/s/b.js"],
        )
        .expect("eslint keeps its contract");
        assert!(findings.is_empty());
        let dirty =
            parse_eslint(ESLINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.js"]).expect("dirty");
        assert_eq!(dirty[0].finding.tool_id, "eslint");
    }
}
