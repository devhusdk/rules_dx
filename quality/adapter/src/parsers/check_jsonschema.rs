use std::collections::BTreeSet;

use super::{as_text, code_name, point, FileFinding, ParseError};
use crate::json_schema::{self, Filetype};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "check_jsonschema";
/// Rule id of a validation error.
pub const RULE_VALIDATION: &str = "json-schema";
/// Rule id of a file the tool could not read as its filetype.
pub const RULE_PARSE: &str = "parse-error";

/// One path `check-jsonschema` was handed, the owned file it stands for and that file's bytes.
#[derive(Debug, Clone, Copy)]
pub struct CheckJsonschemaTarget<'a> {
    pub reported: &'a str,
    pub file: &'a str,
    pub bytes: &'a [u8],
    pub filetype: Filetype,
}

fn shape(detail: impl Into<String>) -> ParseError {
    ParseError::shape(TOOL, detail)
}

fn strings(report: &serde_json::Value, key: &str) -> Result<Option<Vec<String>>, ParseError> {
    let Some(value) = report.get(key) else {
        return Ok(None);
    };
    let items = value
        .as_array()
        .ok_or_else(|| shape(format!("{key} is not an array")))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| shape(format!("{key} holds a non-string")))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn objects<'a>(
    report: &'a serde_json::Value,
    key: &str,
) -> Result<&'a [serde_json::Value], ParseError> {
    report
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| shape(format!("missing {key} (the report needs -vv)")))
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| shape(format!("{key} is missing or not a string")))
}

fn target<'a, 'b>(
    targets: &'b [CheckJsonschemaTarget<'a>],
    name: &str,
) -> Result<&'b CheckJsonschemaTarget<'a>, ParseError> {
    targets
        .iter()
        .find(|target| target.reported == name)
        .ok_or_else(|| ParseError::UnknownFile {
            tool: TOOL,
            path: name.to_owned(),
        })
}

fn clean(message: &str, targets: &[CheckJsonschemaTarget<'_>]) -> String {
    let mut out = message.to_owned();
    for target in targets {
        if !target.reported.is_empty() && target.reported != target.file {
            out = out.replace(target.reported, target.file);
        }
    }
    out.trim().to_owned()
}

fn finding(file: &str, rule: &str, message: String, line: u64, column: u64) -> FileFinding {
    let (start, end) = point(line, column);
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message,
            severity: ToolSeverity::Error,
            start,
            end,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

/// Parses `check-jsonschema --output-format json -vv`.
///
/// Fails closed: exit 0 needs `status: ok` over exactly the files handed in, exit 1 needs a
/// failing report that accounts for every handed-in file, and exit 1 without a JSON report
/// (an invalid schema or a failed `$ref`) is a process error carrying stderr.
pub fn parse_check_jsonschema(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    targets: &[CheckJsonschemaTarget<'_>],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !matches!(code, Some(0 | 1)) {
        return Err(shape(format!("exit {}: {stderr}", code_name(code))));
    }
    let report = as_text(TOOL, stdout)?.trim();
    if !report.starts_with('{') {
        return Err(shape(format!(
            "exit {} without a JSON report: {stderr}",
            code_name(code)
        )));
    }
    let value: serde_json::Value =
        serde_json::from_str(report).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let passed: BTreeSet<&str> = targets.iter().map(|target| target.reported).collect();
    let status = text(&value, "status")?;
    let mut findings = Vec::new();
    match (code, status) {
        (Some(0), "ok") => {
            let errors = value.get("errors").and_then(serde_json::Value::as_array);
            if errors.is_some_and(|errors| !errors.is_empty()) {
                return Err(shape("exit 0 with errors"));
            }
            let checked = strings(&value, "checked_paths")?
                .ok_or_else(|| shape("missing checked_paths (the report needs -vv)"))?;
            for name in &checked {
                target(targets, name)?;
            }
            let checked: BTreeSet<&str> = checked.iter().map(String::as_str).collect();
            if checked != passed {
                return Err(ParseError::Incomplete {
                    tool: TOOL,
                    detail: format!("checked {} of {} files", checked.len(), passed.len()),
                });
            }
            return Ok(findings);
        }
        (Some(1), "fail") => {}
        (_, other) => {
            return Err(shape(format!(
                "exit {} with status {other:?}",
                code_name(code)
            )))
        }
    }
    let mut seen: BTreeSet<String> = strings(&value, "successes")?
        .ok_or_else(|| shape("missing successes (the report needs -vv)"))?
        .into_iter()
        .collect();
    for name in &seen {
        target(targets, name)?;
    }
    let errors = objects(&value, "errors")?;
    let parse_errors = objects(&value, "parse_errors")?;
    if errors.is_empty() && parse_errors.is_empty() {
        return Err(shape("exit 1 without an error"));
    }
    for error in errors {
        let name = text(error, "filename")?;
        let entry = target(targets, name)?;
        seen.insert(name.to_owned());
        let deep = error
            .get("has_sub_errors")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            .then(|| error.get("best_deep_match"))
            .flatten()
            .filter(|deep| deep.is_object());
        let source = deep.unwrap_or(error);
        let path = text(source, "path")?;
        let message = clean(text(source, "message")?, targets);
        let (line, column) = json_schema::locate(entry.bytes, entry.filetype, path);
        findings.push(finding(
            entry.file,
            RULE_VALIDATION,
            format!("{path}: {message}"),
            line,
            column,
        ));
    }
    for error in parse_errors {
        let name = text(error, "filename")?;
        let entry = target(targets, name)?;
        seen.insert(name.to_owned());
        let tool_message = clean(text(error, "message")?, targets);
        let (line, column, message) = match json_schema::syntax_problem(entry.bytes, entry.filetype)
        {
            Some((line, column, detail)) => (line, column, format!("{detail}: {tool_message}")),
            None => (1, 1, tool_message),
        };
        findings.push(finding(entry.file, RULE_PARSE, message, line, column));
    }
    let seen: BTreeSet<&str> = seen.iter().map(String::as_str).collect();
    if seen != passed {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!(
                "the report accounts for {} of {} files",
                seen.len(),
                passed.len()
            ),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets<'a>(bytes: &'a [u8]) -> Vec<CheckJsonschemaTarget<'a>> {
        vec![
            CheckJsonschemaTarget {
                reported: "/s/a.json",
                file: "a.json",
                bytes,
                filetype: Filetype::Json,
            },
            CheckJsonschemaTarget {
                reported: "/s/b.json",
                file: "b.json",
                bytes: b"{}",
                filetype: Filetype::Json,
            },
        ]
    }

    fn run(stdout: &str, code: i32, bytes: &[u8]) -> Result<Vec<FileFinding>, ParseError> {
        parse_check_jsonschema(stdout.as_bytes(), b"", Some(code), &targets(bytes))
    }

    const CLEAN: &str = r#"{"status":"ok","errors":[],"checked_paths":["/s/b.json","/s/a.json"]}"#;
    const A: &[u8] = b"{\n  \"n\": 1\n}\n";

    #[test]
    fn clean_run_has_no_findings() {
        assert_eq!(run(CLEAN, 0, A).expect("clean"), vec![]);
    }

    #[test]
    fn clean_must_cover_every_file() {
        let short = r#"{"status":"ok","errors":[],"checked_paths":["/s/a.json"]}"#;
        assert!(matches!(
            run(short, 0, A),
            Err(ParseError::Incomplete { .. })
        ));
        let extra =
            r#"{"status":"ok","errors":[],"checked_paths":["/s/a.json","/s/b.json","/s/c.json"]}"#;
        assert!(matches!(
            run(extra, 0, A),
            Err(ParseError::UnknownFile { .. })
        ));
        let no_paths = r#"{"status":"ok","errors":[]}"#;
        assert!(run(no_paths, 0, A).is_err());
    }

    #[test]
    fn dirty_run_places_the_finding() {
        let dirty = r#"{"status":"fail","successes":["/s/b.json"],"errors":[{"filename":"/s/a.json","path":"$.n","message":"1 is not of type 'string'","has_sub_errors":false}],"parse_errors":[]}"#;
        let found = run(dirty, 1, A).expect("dirty");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "a.json");
        assert_eq!(found[0].finding.rule_id, RULE_VALIDATION);
        assert_eq!(found[0].finding.message, "$.n: 1 is not of type 'string'");
        assert_eq!(
            (found[0].finding.start.line, found[0].finding.start.column),
            (2, 3)
        );
    }

    #[test]
    fn sub_errors_use_the_best_deep_match() {
        let dirty = r#"{"status":"fail","successes":["/s/b.json"],"errors":[{"filename":"/s/a.json","path":"$","message":"no match","has_sub_errors":true,"best_deep_match":{"path":"$.n","message":"deep"}}],"parse_errors":[]}"#;
        let found = run(dirty, 1, A).expect("dirty");
        assert_eq!(found[0].finding.message, "$.n: deep");
        assert_eq!(found[0].finding.start.line, 2);
    }

    #[test]
    fn parse_errors_are_findings() {
        let dirty = r#"{"status":"fail","successes":["/s/b.json"],"errors":[],"parse_errors":[{"filename":"/s/a.json","message":"bad /s/a.json"}]}"#;
        let found = run(dirty, 1, b"{\n  \"n\": ,\n}").expect("parse error");
        assert_eq!(found[0].finding.rule_id, RULE_PARSE);
        assert_eq!(found[0].finding.start.line, 2);
        assert!(found[0].finding.message.contains("bad a.json"));
    }

    #[test]
    fn malformed_and_dropped_files_fail_closed() {
        let dropped = r#"{"status":"fail","successes":[],"errors":[{"filename":"/s/a.json","path":"$","message":"m","has_sub_errors":false}],"parse_errors":[]}"#;
        assert!(matches!(
            run(dropped, 1, A),
            Err(ParseError::Incomplete { .. })
        ));
        let unknown = r#"{"status":"fail","successes":["/s/b.json"],"errors":[{"filename":"/s/zzz.json","path":"$","message":"m","has_sub_errors":false}],"parse_errors":[]}"#;
        assert!(matches!(
            run(unknown, 1, A),
            Err(ParseError::UnknownFile { .. })
        ));
        let none = r#"{"status":"fail","successes":["/s/a.json","/s/b.json"],"errors":[],"parse_errors":[]}"#;
        assert!(run(none, 1, A).is_err());
        assert!(run(CLEAN, 1, A).is_err());
        let dirty_zero = r#"{"status":"fail","successes":[],"errors":[],"parse_errors":[]}"#;
        assert!(run(dirty_zero, 0, A).is_err());
        assert!(run("{not json", 0, A).is_err());
        assert!(run("[]", 0, A).is_err());
    }

    #[test]
    fn exit_one_without_a_report_is_a_process_error_with_stderr() {
        let err = parse_check_jsonschema(
            b"",
            b"Failure resolving $ref within schema\n",
            Some(1),
            &targets(A),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Failure resolving $ref"), "{err}");
        assert!(
            parse_check_jsonschema(CLEAN.as_bytes(), b"boom", Some(2), &targets(A))
                .unwrap_err()
                .to_string()
                .contains("boom")
        );
        assert!(parse_check_jsonschema(CLEAN.as_bytes(), b"", None, &targets(A)).is_err());
    }

    #[test]
    fn non_utf8_output_is_rejected() {
        assert!(parse_check_jsonschema(&[0xff, 0xfe], b"", Some(0), &targets(A)).is_err());
    }
}
