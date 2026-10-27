use std::path::{Component, Path, PathBuf};

use super::{check_output_size, code_name, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "redocly";

fn shape(detail: impl Into<String>) -> ParseError {
    ParseError::Shape {
        tool: TOOL,
        detail: detail.into(),
    }
}

/// A path with `.` dropped, `..` folded and separators spelled `/`, or `None` when `..` climbs past the start.
fn lexical(path: &Path) -> Option<String> {
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !clean.pop() {
                    return None;
                }
            }
            other => clean.push(other.as_os_str()),
        }
    }
    Some(dx_path::posix(&clean))
}

/// The staged file a `source.ref` names, spelled as `staged` spells it.
fn staged_file<'a>(cwd: &Path, staged: &[&'a str], reference: &str) -> Option<&'a str> {
    let named = Path::new(reference);
    let absolute = if named.is_absolute() || dx_path::drive_rooted(reference) {
        named.to_path_buf()
    } else {
        cwd.join(reference.replace('\\', "/"))
    };
    let wanted = lexical(&absolute)?;
    staged
        .iter()
        .copied()
        .find(|file| lexical(Path::new(file)).is_some_and(|known| known == wanted))
}

/// The workspace spelling of a staged file: its path under `cwd`, else the path itself.
fn display(cwd: &Path, file: &str) -> String {
    Path::new(file)
        .strip_prefix(cwd)
        .map(dx_path::posix)
        .unwrap_or_else(|_| dx_path::posix(Path::new(file)))
}

/// Findings from one `redocly lint --format=json` run over the single root `root`.
///
/// `staged` lists every staged spec file (the root, refs and dep files), spelled as absolute
/// paths. A problem in a fragment lands on the root at line 1, prefixed with the fragment
/// location, since a fragment is never an owned source. Output that does not account for
/// every counted problem, or names an unstaged file, is never read as clean.
pub fn parse_redocly(
    stdout: &[u8],
    code: Option<i32>,
    root: &str,
    cwd: &Path,
    staged: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(shape(format!(
            "exit {} with no JSON report",
            code_name(code)
        )));
    }
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let totals = value.get("totals").ok_or_else(|| shape("missing totals"))?;
    let count = |name: &str| {
        totals
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| shape(format!("missing totals.{name}")))
    };
    let errors = count("errors")?;
    let warnings = count("warnings")?;
    let problems = value
        .get("problems")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| shape("missing problems"))?;
    let root_display = display(cwd, root);
    let mut findings = Vec::with_capacity(problems.len());
    let mut seen_errors = 0u64;
    let mut seen_warnings = 0u64;
    for problem in problems {
        let severity = match problem.get("severity").and_then(serde_json::Value::as_str) {
            Some("error") => {
                seen_errors += 1;
                ToolSeverity::Error
            }
            Some("warn") => {
                seen_warnings += 1;
                ToolSeverity::Warning
            }
            other => return Err(shape(format!("unknown severity {other:?}"))),
        };
        let rule = problem
            .get("ruleId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape("missing ruleId"))?;
        let message = problem
            .get("message")
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| shape("missing message"))?;
        let location = problem
            .get("location")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first());
        let (line, column, message) = match location {
            None => (1, 1, message.to_owned()),
            Some(location) => {
                let reference = location
                    .get("source")
                    .and_then(|source| source.get("ref"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| shape("missing location source"))?;
                let file =
                    staged_file(cwd, staged, reference).ok_or_else(|| ParseError::UnknownFile {
                        tool: TOOL,
                        path: reference.to_owned(),
                    })?;
                let start = location.get("start");
                let at = |field: &str| {
                    start
                        .and_then(|start| start.get(field))
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(1)
                        .max(1)
                };
                let (line, column) = (at("line"), at("col"));
                if display(cwd, file) == root_display {
                    (line, column, message.to_owned())
                } else {
                    (
                        1,
                        1,
                        format!("{}:{line}:{column}: {message}", display(cwd, file)),
                    )
                }
            }
        };
        let (start, end) = point(line, column);
        findings.push(FileFinding {
            file: root.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule.to_owned(),
                message,
                severity,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Utf16,
                related: Vec::new(),
            },
        });
    }
    if seen_errors != errors || seen_warnings != warnings {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!(
                "totals count {errors} errors and {warnings} warnings but the report lists {seen_errors} and {seen_warnings}"
            ),
        });
    }
    match (code, errors) {
        (Some(0), 0) | (Some(1), 1..) => Ok(findings),
        (Some(1), 0) => Err(shape("exit 1 with no errors")),
        (Some(0), _) => Err(shape(format!("exit 0 with {errors} errors"))),
        _ => Err(shape(format!("exit {}", code_name(code)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/s/api/openapi.yaml";
    const FRAGMENT: &str = "/s/api/schemas/pet.yaml";
    const STAGED: &[&str] = &[ROOT, FRAGMENT];

    fn report(errors: u64, warnings: u64, problems: &str) -> Vec<u8> {
        format!(
            r#"{{"totals":{{"errors":{errors},"warnings":{warnings},"ignored":0}},"version":"2.71.1","problems":[{problems}]}}"#
        )
        .into_bytes()
    }

    fn problem(rule: &str, severity: &str, reference: &str, line: u64, col: u64) -> String {
        format!(
            r##"{{"ruleId":"{rule}","severity":"{severity}","message":"bad {rule}","location":[{{"source":{{"ref":"{reference}"}},"pointer":"#/x","reportOnKey":false,"start":{{"line":{line},"col":{col}}},"end":{{"line":{line},"col":{col}}}}}],"suggest":[]}}"##
        )
    }

    fn parse(stdout: &[u8], code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_redocly(stdout, code, ROOT, Path::new("/s"), STAGED)
    }

    #[test]
    fn redocly_clean_report_is_empty() {
        assert!(parse(&report(0, 0, ""), Some(0)).expect("clean").is_empty());
    }

    #[test]
    fn redocly_root_findings_keep_their_position() {
        let body = problem(
            "operation-operationId-unique",
            "error",
            "api/openapi.yaml",
            9,
            7,
        );
        let findings = parse(&report(1, 0, &body), Some(1)).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, ROOT);
        assert_eq!(findings[0].finding.rule_id, "operation-operationId-unique");
        assert_eq!(findings[0].finding.start.line, 9);
        assert_eq!(findings[0].finding.start.column, 7);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn redocly_fragment_findings_land_on_the_root() {
        let body = problem("struct", "error", "api/schemas/pet.yaml", 4, 11);
        let findings = parse(&report(1, 0, &body), Some(1)).expect("parsed");
        assert_eq!(findings[0].file, ROOT);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(
            findings[0].finding.message,
            "api/schemas/pet.yaml:4:11: bad struct"
        );
    }

    #[test]
    fn redocly_accepts_absolute_and_windows_spellings() {
        let body = problem("struct", "warn", "/s/api/./openapi.yaml", 2, 1);
        assert_eq!(parse(&report(0, 1, &body), Some(0)).expect("abs").len(), 1);
        let body = problem("struct", "warn", "api\\\\schemas\\\\pet.yaml", 2, 1);
        let findings = parse(&report(0, 1, &body), Some(0)).expect("windows");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert!(findings[0]
            .finding
            .message
            .starts_with("api/schemas/pet.yaml:2:1"));
    }

    #[test]
    fn redocly_unstaged_files_fail() {
        let body = problem("struct", "error", "api/elsewhere.yaml", 1, 1);
        assert!(matches!(
            parse(&report(1, 0, &body), Some(1)),
            Err(ParseError::UnknownFile { .. })
        ));
        let body = problem("struct", "error", "../../etc/passwd", 1, 1);
        assert!(parse(&report(1, 0, &body), Some(1)).is_err());
    }

    #[test]
    fn redocly_truncated_reports_fail() {
        let body = problem("struct", "error", "api/openapi.yaml", 1, 1);
        assert!(matches!(
            parse(&report(3, 0, &body), Some(1)),
            Err(ParseError::Incomplete { .. })
        ));
        assert!(matches!(
            parse(&report(1, 2, &body), Some(1)),
            Err(ParseError::Incomplete { .. })
        ));
    }

    #[test]
    fn redocly_exit_codes_must_match_the_report() {
        let body = problem("struct", "error", "api/openapi.yaml", 1, 1);
        assert!(parse(&report(1, 0, &body), Some(0)).is_err());
        assert!(parse(&report(0, 0, ""), Some(1)).is_err());
        assert!(parse(&report(0, 0, ""), Some(2)).is_err());
        assert!(parse(&report(0, 0, ""), None).is_err());
        let warn = problem("struct", "warn", "api/openapi.yaml", 1, 1);
        assert_eq!(parse(&report(0, 1, &warn), Some(0)).expect("warn").len(), 1);
    }

    #[test]
    fn redocly_rejects_empty_and_foreign_output() {
        assert!(parse(b"", Some(1)).is_err());
        assert!(parse(b"  \n", Some(0)).is_err());
        assert!(parse(b"Failed to parse API description", Some(1)).is_err());
        assert!(parse(br#"{"problems":[]}"#, Some(0)).is_err());
        assert!(parse(br#"{"totals":{"errors":0,"warnings":0}}"#, Some(0)).is_err());
        let body = problem("struct", "fatal", "api/openapi.yaml", 1, 1);
        assert!(parse(&report(1, 0, &body), Some(1)).is_err());
        assert!(parse(&[0xff], Some(0)).is_err());
    }

    #[test]
    fn redocly_problems_without_a_location_land_on_line_one() {
        let body =
            r#"{"ruleId":"info-contact","severity":"warn","message":"no contact","location":[]}"#;
        let findings = parse(&report(0, 1, body), Some(0)).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 1);
    }
}
