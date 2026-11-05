use super::{as_text, code_name, known_spelled, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "debride";

/// Splits a `path:line` or `path:line-end` location into its path and first line.
fn split_location(location: &str) -> Option<(&str, u64)> {
    let (path, span) = location.rsplit_once(':')?;
    let line = span.split('-').next()?.parse::<u64>().ok()?;
    (!path.is_empty() && line > 0).then_some((path, line))
}

/// Parses `debride --rails --json <files>`: one JSON object whose `missing` maps each class to
/// `[method, "path:line[-end]"]` pairs for methods nothing in the scanned files calls.
///
/// Every pair is an info finding at the method's first line. debride prints nothing on stderr
/// when it reads every file, so any stderr means it skipped input and the result is incomplete.
pub fn parse_debride(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {stderr}", code_name(code)),
        ));
    }
    if !stderr.is_empty() {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("debride skipped input: {stderr}"),
        });
    }
    let report = as_text(TOOL, stdout)?.trim();
    let value: serde_json::Value =
        serde_json::from_str(report).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let missing = value
        .get("missing")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| ParseError::shape(TOOL, "missing object"))?;
    let mut findings = Vec::new();
    for (class, methods) in missing {
        let methods = methods
            .as_array()
            .ok_or_else(|| ParseError::shape(TOOL, format!("{class} is not a list")))?;
        for entry in methods {
            let pair = entry
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| {
                    ParseError::shape(
                        TOOL,
                        format!("{class} entry is not a [name, location] pair"),
                    )
                })?;
            let name = pair[0]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| ParseError::shape(TOOL, format!("{class} entry has no name")))?;
            let location = pair[1].as_str().ok_or_else(|| {
                ParseError::shape(TOOL, format!("{class}#{name} has no location"))
            })?;
            let (path, line) = split_location(location).ok_or_else(|| {
                ParseError::shape(
                    TOOL,
                    format!("bad location {location:?} for {class}#{name}"),
                )
            })?;
            let checked = known_spelled(TOOL, files, path)?;
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: "uncalled_method".to_owned(),
                    message: format!("{class}#{name} might not be called"),
                    severity: ToolSeverity::Warning,
                    start: TextPosition { line, column: 1 },
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf8,
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

    const ONE: &str = r#"{"missing":{"Foo":[["bar","lib/foo.rb:2-4"]]}}"#;

    #[test]
    fn debride_clean_run_is_clean() {
        assert!(
            parse_debride(br#"{"missing":{}}"#, b"", Some(0), &["lib/foo.rb"])
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn debride_one_uncalled_method_is_one_info_finding() {
        let findings =
            parse_debride(ONE.as_bytes(), b"", Some(0), &["lib/foo.rb"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.file, "lib/foo.rb");
        assert_eq!(finding.finding.tool_id, "debride");
        assert_eq!(finding.finding.rule_id, "uncalled_method");
        assert_eq!(finding.finding.message, "Foo#bar might not be called");
        assert_eq!(finding.finding.severity, ToolSeverity::Warning);
        assert_eq!(finding.finding.start, TextPosition { line: 2, column: 1 });
        assert_eq!(finding.finding.end, None);
    }

    #[test]
    fn debride_accepts_a_single_line_location() {
        let single = r#"{"missing":{"Foo":[["bar","lib/foo.rb:7"]]}}"#;
        let findings =
            parse_debride(single.as_bytes(), b"", Some(0), &["lib/foo.rb"]).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 7);
    }

    #[test]
    fn debride_rejects_unchecked_files() {
        let err = parse_debride(ONE.as_bytes(), b"", Some(0), &["other.rb"]).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }

    #[test]
    fn debride_rejects_bad_exit_codes() {
        assert!(parse_debride(ONE.as_bytes(), b"", Some(1), &["lib/foo.rb"]).is_err());
        assert!(parse_debride(ONE.as_bytes(), b"", None, &["lib/foo.rb"]).is_err());
    }

    #[test]
    fn debride_skipped_input_is_incomplete() {
        let err = parse_debride(
            br#"{"missing":{}}"#,
            b"  skipping lib/foo.rb: unexpected end-of-input",
            Some(0),
            &["lib/foo.rb"],
        )
        .expect_err("skipped");
        assert!(matches!(err, ParseError::Incomplete { .. }), "{err}");
    }

    #[test]
    fn debride_rejects_malformed_output() {
        assert!(parse_debride(b"", b"", Some(0), &["lib/foo.rb"]).is_err());
        assert!(parse_debride(b"Foo\n  bar lib/foo.rb:2", b"", Some(0), &["lib/foo.rb"]).is_err());
        assert!(parse_debride(br#"{"other":{}}"#, b"", Some(0), &["lib/foo.rb"]).is_err());
        assert!(parse_debride(
            br#"{"missing":{"Foo":["bar"]}}"#,
            b"",
            Some(0),
            &["lib/foo.rb"]
        )
        .is_err());
        assert!(parse_debride(
            br#"{"missing":{"Foo":[["bar",null]]}}"#,
            b"",
            Some(0),
            &["lib/foo.rb"]
        )
        .is_err());
        assert!(parse_debride(
            br#"{"missing":{"Foo":[["bar","lib/foo.rb:x"]]}}"#,
            b"",
            Some(0),
            &["lib/foo.rb"]
        )
        .is_err());
        assert!(parse_debride(&[0xff], b"", Some(0), &["lib/foo.rb"]).is_err());
    }
}
