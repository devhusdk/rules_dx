use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{
    ColumnEncoding, Finding, Related, RelatedLocation, Suggestion, TextPosition, ToolSeverity,
};

const MAX_RELATED: usize = 32;

#[derive(Debug, Deserialize)]
struct ClippyMessage {
    message: String,
    code: Option<ClippyCode>,
    level: String,
    #[serde(default)]
    spans: Vec<ClippySpan>,
    #[serde(default)]
    children: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ClippyCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct ClippySpan {
    file_name: String,
    byte_start: u64,
    byte_end: u64,
    line_start: u64,
    line_end: u64,
    column_start: u64,
    column_end: u64,
    is_primary: bool,
    #[serde(default)]
    label: Option<String>,
    suggested_replacement: Option<String>,
    suggestion_applicability: Option<String>,
}

fn clippy_level(tool: &'static str, level: &str) -> Result<ToolSeverity, ParseError> {
    match level {
        "warning" => Ok(ToolSeverity::Warning),
        "error" => Ok(ToolSeverity::Error),
        _ => Err(ParseError::Shape {
            tool,
            detail: format!("unknown level: {level}"),
        }),
    }
}

pub fn parse_clippy(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("clippy", stderr)?;
    parse_rust_diagnostics("clippy", stderr, code, files)
}

pub fn parse_rustc(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("rustc", stderr)?;
    parse_rust_diagnostics("rustc", stderr, code, files)
}

/// One rustc diagnostic read from an action's stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDiagnostic {
    pub severity: ToolSeverity,
    pub code: Option<String>,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

/// Reads rustc diagnostics from action stderr, as JSON lines or rendered text.
pub fn parse_rustc_action_output(stderr: &[u8]) -> Result<Vec<ActionDiagnostic>, ParseError> {
    check_output_size("rustc", stderr)?;
    let text = String::from_utf8_lossy(stderr);
    let mut found = Vec::new();
    let mut saw_json = false;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value
            .get("$message_type")
            .and_then(serde_json::Value::as_str)
            != Some("diagnostic")
        {
            continue;
        }
        saw_json = true;
        let Ok(diagnostic) = serde_json::from_value::<ClippyMessage>(value) else {
            continue;
        };
        let severity = if diagnostic.level.starts_with("error") {
            ToolSeverity::Error
        } else if diagnostic.level == "warning" {
            ToolSeverity::Warning
        } else {
            continue;
        };
        let span = diagnostic
            .spans
            .iter()
            .find(|span| span.is_primary)
            .or_else(|| diagnostic.spans.first());
        if span.is_none()
            && (diagnostic.message.starts_with("aborting due to")
                || diagnostic.message.ends_with(" emitted"))
        {
            continue;
        }
        found.push(ActionDiagnostic {
            severity,
            code: diagnostic.code.map(|code| code.code),
            message: diagnostic.message,
            file: span.map(|span| span.file_name.clone()),
            line: span.map(|span| span.line_start).filter(|line| *line > 0),
            column: span
                .map(|span| span.column_start)
                .filter(|column| *column > 0),
        });
    }
    if !saw_json {
        found = parse_rendered(&strip_ansi(&text));
    }
    Ok(found)
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            plain.push(ch);
            continue;
        }
        if chars.next() == Some('[') {
            for next in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&next) {
                    break;
                }
            }
        }
    }
    plain
}

fn rendered_header(line: &str) -> Option<(ToolSeverity, Option<String>, &str)> {
    let (severity, rest) = line
        .strip_prefix("error")
        .map(|rest| (ToolSeverity::Error, rest))
        .or_else(|| {
            line.strip_prefix("warning")
                .map(|rest| (ToolSeverity::Warning, rest))
        })?;
    let (code, rest) = match rest.strip_prefix('[') {
        Some(bracketed) => {
            let (code, tail) = bracketed.split_once(']')?;
            if code.is_empty() || code.contains(char::is_whitespace) {
                return None;
            }
            (Some(code.to_owned()), tail)
        }
        None => (None, rest),
    };
    let message = rest.strip_prefix(": ")?.trim_end();
    (!message.is_empty()).then_some((severity, code, message))
}

fn rendered_location(line: &str) -> Option<(String, u64, u64)> {
    let location = line.trim_start().strip_prefix("--> ")?.trim();
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next().filter(|file| !file.is_empty())?;
    Some((file.to_owned(), line, column))
}

fn parse_rendered(text: &str) -> Vec<ActionDiagnostic> {
    let mut found = Vec::new();
    let mut open = false;
    for line in text.lines() {
        if let Some((severity, code, message)) = rendered_header(line) {
            found.push(ActionDiagnostic {
                severity,
                code,
                message: message.to_owned(),
                file: None,
                line: None,
                column: None,
            });
            open = true;
            continue;
        }
        if line.trim().is_empty() {
            open = false;
            continue;
        }
        if !open {
            continue;
        }
        if let Some((file, line, column)) = rendered_location(line) {
            if let Some(last) = found.last_mut() {
                last.file = Some(file);
                last.line = Some(line).filter(|line| *line > 0);
                last.column = Some(column).filter(|column| *column > 0);
            }
            open = false;
        }
    }
    found.retain(|item| {
        !item.message.starts_with("could not compile")
            && (item.file.is_some()
                || !(item.message.starts_with("aborting due to")
                    || item.message.ends_with(" emitted")))
    });
    found
}

fn parse_rust_diagnostics(
    tool: &'static str,
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stderr)?;
    let stderr_text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for line in stderr_text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|err| ParseError::Shape {
                tool,
                detail: format!("unparsable line: {err}"),
            })?;
        if value
            .get("$message_type")
            .and_then(serde_json::Value::as_str)
            != Some("diagnostic")
        {
            continue;
        }
        let diagnostic: ClippyMessage =
            serde_json::from_value(value).map_err(|err| ParseError::Shape {
                tool,
                detail: format!("malformed diagnostic: {err}"),
            })?;
        let Some(span) = diagnostic
            .spans
            .iter()
            .filter(|span| files.contains(&span.file_name.as_str()))
            .find(|span| span.is_primary)
            .or_else(|| {
                diagnostic
                    .spans
                    .iter()
                    .find(|span| files.contains(&span.file_name.as_str()))
            })
        else {
            if diagnostic.spans.is_empty() {
                skipped.push(diagnostic.message);
            } else {
                return Err(ParseError::Shape {
                    tool,
                    detail: format!(
                        "diagnostic outside the checked files: {}",
                        diagnostic.message
                    ),
                });
            }
            continue;
        };
        if span.line_start == 0
            || span.column_start == 0
            || span.line_end == 0
            || span.column_end == 0
        {
            return Err(ParseError::Shape {
                tool,
                detail: format!("zero position in {}", diagnostic.message),
            });
        }
        let children = lenient_children(&diagnostic.children);
        let mut suggestions = Vec::new();
        collect_suggestions(tool, &children, &span.file_name, &mut suggestions)?;
        let related = collect_related(&diagnostic.spans, &children);
        let checked = known(tool, files, &span.file_name)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: tool.to_owned(),
                rule_id: diagnostic.code.map_or_else(String::new, |code| code.code),
                message: diagnostic.message,
                severity: clippy_level(tool, &diagnostic.level)?,
                start: TextPosition {
                    line: span.line_start,
                    column: span.column_start,
                },
                end: Some(TextPosition {
                    line: span.line_end,
                    column: span.column_end,
                }),
                suggestions,
                encoding: ColumnEncoding::Scalars,
                related,
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        let detail = skipped.first().map_or_else(
            || format!("exit {} with no diagnostics", code_name(code)),
            |message| {
                format!(
                    "exit {} with no placed diagnostics: {message}",
                    code_name(code)
                )
            },
        );
        return Err(ParseError::Shape { tool, detail });
    }
    Ok(findings)
}

fn lenient_children(children: &[serde_json::Value]) -> Vec<ClippyMessage> {
    children
        .iter()
        .filter_map(|child| serde_json::from_value(child.clone()).ok())
        .collect()
}

fn related_location(span: &ClippySpan) -> Option<RelatedLocation> {
    if span.line_start == 0 || span.column_start == 0 {
        return None;
    }
    let end = (span.line_end > 0 && span.column_end > 0).then_some(TextPosition {
        line: span.line_end,
        column: span.column_end,
    });
    Some(RelatedLocation {
        file: span.file_name.clone(),
        start: TextPosition {
            line: span.line_start,
            column: span.column_start,
        },
        end,
    })
}

fn collect_related(spans: &[ClippySpan], children: &[ClippyMessage]) -> Vec<Related> {
    let mut out = Vec::new();
    push_labels(spans, &mut out);
    push_children(children, &mut out);
    out.truncate(MAX_RELATED);
    out
}

fn push_labels(spans: &[ClippySpan], out: &mut Vec<Related>) {
    for span in spans {
        let Some(label) = span.label.as_deref().filter(|label| !label.is_empty()) else {
            continue;
        };
        let mut item = Related::new("label", label);
        item.location = related_location(span);
        out.push(item);
    }
}

fn push_children(children: &[ClippyMessage], out: &mut Vec<Related>) {
    for child in children {
        if child.level == "note" || child.level == "help" {
            let span = child
                .spans
                .iter()
                .find(|span| span.is_primary)
                .or_else(|| child.spans.first());
            let mut item = Related::new(&child.level, &child.message);
            item.location = span.and_then(related_location);
            if let Some(span) = span {
                item.replacement.clone_from(&span.suggested_replacement);
                if span.suggested_replacement.is_some() {
                    item.applicability = span.suggestion_applicability.clone().unwrap_or_default();
                }
            }
            if !item.message.is_empty() {
                out.push(item);
            }
        }
        push_children(&lenient_children(&child.children), out);
    }
}

fn collect_suggestions(
    tool: &'static str,
    messages: &[ClippyMessage],
    file: &str,
    out: &mut Vec<Suggestion>,
) -> Result<(), ParseError> {
    for message in messages {
        for span in &message.spans {
            if let (Some(replacement), Some(applicability)) =
                (&span.suggested_replacement, &span.suggestion_applicability)
            {
                if applicability == "MachineApplicable" && span.file_name == file {
                    if span.byte_start > span.byte_end {
                        return Err(ParseError::Shape {
                            tool,
                            detail: format!("inverted suggestion span in {}", message.message),
                        });
                    }
                    out.push(Suggestion {
                        start: span.byte_start,
                        end: span.byte_end,
                        replacement: replacement.as_bytes().to_vec(),
                    });
                }
            }
        }
        collect_suggestions(tool, &lenient_children(&message.children), file, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(message: &str, level: &str, spans: &str, children: &str) -> String {
        format!(
            r#"{{"$message_type": "diagnostic", "message": {message}, "code": null, "level": "{level}", "spans": [{spans}], "children": [{children}], "rendered": null}}"#
        )
    }

    fn span(file: &str, line: u64, column: u64, primary: bool) -> String {
        format!(
            concat!(
                r#"{{"file_name": "{file}", "byte_start": 0, "byte_end": 1, "#,
                r#""line_start": {line}, "line_end": {line}, "#,
                r#""column_start": {column}, "column_end": {column}, "#,
                r#""is_primary": {primary}, "text": [], "label": null, "#,
                r#""suggested_replacement": null, "suggestion_applicability": null, "expansion": null}}"#
            ),
            file = file,
            line = line,
            column = column,
            primary = primary
        )
    }

    const CLIPPY_LINT: &str = r#"{"$message_type":"diagnostic","message":"length comparison to zero","code":{"code":"clippy::len_zero","explanation":null},"level":"warning","spans":[{"file_name":"/s/len.rs","byte_start":19,"byte_end":33,"line_start":2,"line_end":2,"column_start":8,"column_end":22,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"using `is_empty` is clearer and more explicit","code":null,"level":"help","spans":[{"file_name":"/s/len.rs","byte_start":19,"byte_end":33,"line_start":2,"line_end":2,"column_start":8,"column_end":22,"is_primary":true,"text":[],"label":null,"suggested_replacement":"\"x\".is_empty()","suggestion_applicability":"MachineApplicable","expansion":null}],"children":[],"rendered":null}],"rendered":null}
{"$message_type":"diagnostic","message":"1 warning emitted","code":null,"level":"warning","spans":[],"children":[],"rendered":null}"#;

    const CLIPPY_BROKEN: &str = r#"{"$message_type":"diagnostic","message":"this file contains an unclosed delimiter","code":null,"level":"error","spans":[{"file_name":"/s/broken.rs","byte_start":11,"byte_end":11,"line_start":1,"line_end":1,"column_start":12,"column_end":12,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}
{"$message_type":"diagnostic","message":"aborting due to 1 previous error","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

    #[test]
    fn action_output_reads_errors_and_skips_text_and_summaries() {
        let mut input = String::from("compiling\n");
        input.push_str(CLIPPY_BROKEN);
        input.push_str("\nnot json\n");
        let found = parse_rustc_action_output(input.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity, ToolSeverity::Error);
        assert_eq!(found[0].file.as_deref(), Some("/s/broken.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(1), Some(12)));
    }

    #[test]
    fn action_output_keeps_spanless_link_errors() {
        let line = r#"{"$message_type":"diagnostic","message":"linking with `cc` failed: exit status: 1","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;
        let found = parse_rustc_action_output(line.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, None);
        assert_eq!(found[0].line, None);
    }

    const RENDERED_E0308: &str = "\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror[E0308]\u{1b}[0m\u{1b}[0m\u{1b}[1m: mismatched types\u{1b}[0m
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m--> \u{1b}[0m\u{1b}[0mcli/cli/src/main.rs:12:26\u{1b}[0m
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m
\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m12\u{1b}[0m \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m fn _dx_broken() -> u32 { \"x\" }
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m                    \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9m^^^\u{1b}[0m expected `u32`, found `&str`

\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror\u{1b}[0m\u{1b}[0m\u{1b}[1m: aborting due to 1 previous error\u{1b}[0m

\u{1b}[0m\u{1b}[1mFor more information about this error, try `rustc --explain E0308`.\u{1b}[0m
";

    #[test]
    fn action_output_reads_rendered_errors_with_ansi_codes() {
        let found = parse_rustc_action_output(RENDERED_E0308.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity, ToolSeverity::Error);
        assert_eq!(found[0].code.as_deref(), Some("E0308"));
        assert_eq!(found[0].message, "mismatched types");
        assert_eq!(found[0].file.as_deref(), Some("cli/cli/src/main.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(12), Some(26)));
    }

    #[test]
    fn action_output_reads_rendered_warnings_and_errors_in_order() {
        let input = "warning: length comparison to zero
 --> src/len.rs:2:8
  |
2 |     if \"x\".len() == 0 {}
  |        ^^^^^^^^^^^^^^ help: using `is_empty` is clearer and more explicit
  = note: `#[warn(clippy::len_zero)]` on by default

error[E0425]: cannot find value `y` in this scope
   --> C:\\work\\src\\lib.rs:7:5
    |
note: the item is defined here
   --> src/other.rs:1:1

warning: 1 warning emitted

error: could not compile `app` (lib) due to 1 previous error
";
        let found = parse_rustc_action_output(input.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].severity, ToolSeverity::Warning);
        assert_eq!(found[0].code, None);
        assert_eq!(found[0].message, "length comparison to zero");
        assert_eq!(found[0].file.as_deref(), Some("src/len.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(2), Some(8)));
        assert_eq!(found[1].severity, ToolSeverity::Error);
        assert_eq!(found[1].code.as_deref(), Some("E0425"));
        assert_eq!(found[1].file.as_deref(), Some("C:\\work\\src\\lib.rs"));
        assert_eq!((found[1].line, found[1].column), (Some(7), Some(5)));
    }

    #[test]
    fn action_output_keeps_rendered_spanless_link_errors() {
        let input = "error: linking with `cc` failed: exit status: 1\n  |\n  = note: ld: missing\n\nerror: aborting due to 1 previous error\n";
        let found = parse_rustc_action_output(input.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].message, "linking with `cc` failed: exit status: 1");
        assert_eq!((found[0].file.as_deref(), found[0].line), (None, None));
    }

    #[test]
    fn action_output_ignores_indented_and_unrelated_text() {
        let input = "  error: from a build script\nerror in build script\nnote: --> a.rs:1:1\n";
        let found = parse_rustc_action_output(input.as_bytes()).expect("parsed");
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn action_output_prefers_json_over_rendered_lines() {
        let mut input = String::from("error[E0308]: rendered copy\n --> src/x.rs:9:9\n\n");
        input.push_str(CLIPPY_BROKEN);
        let found = parse_rustc_action_output(input.as_bytes()).expect("parsed");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].message, "this file contains an unclosed delimiter");
        assert_eq!(found[0].file.as_deref(), Some("/s/broken.rs"));
    }

    #[test]
    fn clippy_places_diagnostics_and_harvests_suggestions() {
        let findings =
            parse_clippy(CLIPPY_LINT.as_bytes(), Some(0), &["/s/len.rs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "clippy::len_zero");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 2, column: 8 },
                Some(TextPosition {
                    line: 2,
                    column: 22
                })
            )
        );
        assert_eq!(
            findings[0].finding.suggestions,
            vec![Suggestion {
                start: 19,
                end: 33,
                replacement: b"\"x\".is_empty()".to_vec(),
            }]
        );

        let broken =
            parse_clippy(CLIPPY_BROKEN.as_bytes(), Some(1), &["/s/broken.rs"]).expect("parsed");
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].finding.rule_id, "");
        assert_eq!(broken[0].finding.severity, ToolSeverity::Error);

        assert!(parse_clippy(b"", Some(0), &["/s/clean.rs"])
            .expect("parsed")
            .is_empty());
        let err = parse_clippy(
            CLIPPY_LINT.lines().nth(1).expect("summary").as_bytes(),
            Some(1),
            &["/s/len.rs"],
        )
        .expect_err("unplaced failure");
        assert!(err.to_string().contains("1 warning emitted"));
    }

    const RUSTC_TYPE_ERROR: &str = r#"{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"error","spans":[{"file_name":"/s/type.rs","byte_start":27,"byte_end":32,"line_start":2,"line_end":2,"column_start":9,"column_end":14,"is_primary":true,"text":[],"label":"expected `i32`, found `&str`","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}
{"$message_type":"diagnostic","message":"aborting due to 1 previous error","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

    #[test]
    fn rustc_parses_the_shared_diagnostic_grammar_as_typecheck() {
        let findings =
            parse_rustc(RUSTC_TYPE_ERROR.as_bytes(), Some(1), &["/s/type.rs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/type.rs");
        assert_eq!(findings[0].finding.tool_id, "rustc");
        assert_eq!(findings[0].finding.rule_id, "E0308");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 2, column: 9 },
                Some(TextPosition {
                    line: 2,
                    column: 14
                })
            )
        );
        assert!(parse_rustc(b"", Some(0), &["/s/clean.rs"])
            .expect("parsed")
            .is_empty());
        let err = parse_rustc(
            RUSTC_TYPE_ERROR.lines().nth(1).expect("summary").as_bytes(),
            Some(1),
            &["/s/type.rs"],
        )
        .expect_err("unplaced failure");
        assert!(err.to_string().contains("rustc"));
        assert!(err.to_string().contains("aborting due to"));
    }

    #[test]
    fn clippy_rejects_bytes_lines_and_unplaced_diagnostics() {
        assert!(parse_clippy(b"\xff", Some(1), &["/s/x.rs"]).is_err());
        assert!(parse_clippy(b"  \n", Some(0), &["/s/x.rs"])
            .expect("parsed")
            .is_empty());
        assert!(parse_clippy(b"hello\n", Some(0), &["/s/x.rs"]).is_err());
        let artifact = "{\"$message_type\": \"artifact\", \"x\": 1}\n";
        assert!(parse_clippy(artifact.as_bytes(), Some(0), &["/s/x.rs"])
            .expect("parsed")
            .is_empty());
        let bare = "{\"$message_type\": \"diagnostic\"}\n";
        assert!(parse_clippy(bare.as_bytes(), Some(0), &["/s/x.rs"]).is_err());
        let outside = diagnostic(
            "\"elsewhere\"",
            "warning",
            &span("/other.rs", 1, 1, true),
            "",
        );
        assert!(parse_clippy(outside.as_bytes(), Some(0), &["/s/x.rs"]).is_err());
        let zero = diagnostic("\"zero\"", "warning", &span("/s/x.rs", 1, 0, true), "");
        assert!(parse_clippy(zero.as_bytes(), Some(0), &["/s/x.rs"]).is_err());
        let noted = diagnostic("\"noted\"", "note", &span("/s/x.rs", 1, 1, true), "");
        assert!(parse_clippy(noted.as_bytes(), Some(0), &["/s/x.rs"]).is_err());
        let err = parse_clippy(b"", Some(1), &["/s/x.rs"]).expect_err("empty failure");
        assert!(err.to_string().contains('1'));
    }

    #[test]
    fn clippy_keeps_only_machine_applicable_suggestions() {
        let maybe = format!(concat!(
            r#"{{"message": "maybe", "code": null, "level": "help", "spans": ["#,
            r#"{{"file_name": "/s/x.rs", "byte_start": 0, "byte_end": 1, "#,
            r#""line_start": 1, "line_end": 1, "column_start": 1, "column_end": 2, "#,
            r#""is_primary": true, "text": [], "label": null, "#,
            r#""suggested_replacement": "y", "suggestion_applicability": "MaybeIncorrect", "#,
            r#""expansion": null}}], "children": [], "rendered": null}}"#
        ));
        let plain = format!(concat!(
            r#"{{"message": "plain", "code": null, "level": "help", "spans": ["#,
            r#"{{"file_name": "/s/x.rs", "byte_start": 0, "byte_end": 1, "#,
            r#""line_start": 1, "line_end": 1, "column_start": 1, "column_end": 2, "#,
            r#""is_primary": true, "text": [], "label": null, "#,
            r#""suggested_replacement": null, "suggestion_applicability": null, "#,
            r#""expansion": null}}], "children": [], "rendered": null}}"#
        ));
        let secondary = span("/s/x.rs", 2, 3, false);
        let line = diagnostic(
            "\"lint\"",
            "warning",
            &secondary,
            &format!("{maybe}, {plain}"),
        );
        let findings = parse_clippy(line.as_bytes(), Some(0), &["/s/x.rs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 2, column: 3 },
                Some(TextPosition { line: 2, column: 3 })
            )
        );
        assert!(findings[0].finding.suggestions.is_empty());
        let inverted = format!(concat!(
            r#"{{"message": "fix", "code": null, "level": "help", "spans": ["#,
            r#"{{"file_name": "/s/x.rs", "byte_start": 5, "byte_end": 2, "#,
            r#""line_start": 1, "line_end": 1, "column_start": 1, "column_end": 2, "#,
            r#""is_primary": true, "text": [], "label": null, "#,
            r#""suggested_replacement": "y", "suggestion_applicability": "MachineApplicable", "#,
            r#""expansion": null}}], "children": [], "rendered": null}}"#
        ));
        let bad = diagnostic(
            "\"lint\"",
            "warning",
            &span("/s/x.rs", 1, 1, true),
            &inverted,
        );
        assert!(parse_clippy(bad.as_bytes(), Some(0), &["/s/x.rs"]).is_err());
    }

    const CLIPPY_CONTEXT: &str = r#"{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"error","spans":[{"file_name":"/s/a.rs","byte_start":27,"byte_end":32,"line_start":2,"line_end":2,"column_start":9,"column_end":14,"is_primary":true,"text":[],"label":"expected `i32`, found `&str`","suggested_replacement":null,"suggestion_applicability":null,"expansion":null},{"file_name":"/s/b.rs","byte_start":3,"byte_end":6,"line_start":1,"line_end":1,"column_start":4,"column_end":7,"is_primary":false,"text":[],"label":"expected due to this","suggested_replacement":null,"suggestion_applicability":null,"expansion":null},{"file_name":"/s/b.rs","byte_start":9,"byte_end":10,"line_start":1,"line_end":1,"column_start":10,"column_end":11,"is_primary":false,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"required by a bound in `take`","code":null,"level":"note","spans":[{"file_name":"/rustc/abc/library/core/src/x.rs","byte_start":1,"byte_end":2,"line_start":5,"line_end":5,"column_start":1,"column_end":2,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null},{"message":"try converting","code":null,"level":"help","spans":[{"file_name":"/s/a.rs","byte_start":27,"byte_end":32,"line_start":2,"line_end":2,"column_start":9,"column_end":14,"is_primary":true,"text":[],"label":null,"suggested_replacement":"1","suggestion_applicability":"MaybeIncorrect","expansion":null}],"children":[],"rendered":null},{"message":"plain note","code":null,"level":"note","spans":[],"children":[],"rendered":null},{"broken":true}],"rendered":null}"#;

    #[test]
    fn rust_keeps_labels_notes_help_and_related_locations() {
        let findings = parse_rustc(CLIPPY_CONTEXT.as_bytes(), Some(1), &["/s/a.rs", "/s/b.rs"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0].finding;
        assert_eq!(finding.message, "mismatched types");
        assert!(finding.suggestions.is_empty());
        let roles: Vec<(&str, &str)> = finding
            .related
            .iter()
            .map(|item| (item.role.as_str(), item.message.as_str()))
            .collect();
        assert_eq!(
            roles,
            vec![
                ("label", "expected `i32`, found `&str`"),
                ("label", "expected due to this"),
                ("note", "required by a bound in `take`"),
                ("help", "try converting"),
                ("note", "plain note"),
            ]
        );
        assert_eq!(
            finding.related[0].location.as_ref().expect("a").file,
            "/s/a.rs"
        );
        assert_eq!(
            finding.related[1].location.as_ref().expect("b").file,
            "/s/b.rs"
        );
        assert_eq!(finding.related[3].replacement.as_deref(), Some("1"));
        assert_eq!(finding.related[3].applicability, "MaybeIncorrect");
        assert!(finding.related[4].location.is_none());
        assert_eq!(finding.related[2].replacement, None);
    }

    #[test]
    fn rust_without_context_has_no_related_items() {
        let findings =
            parse_clippy(CLIPPY_BROKEN.as_bytes(), Some(1), &["/s/broken.rs"]).expect("parsed");
        assert!(findings[0].finding.related.is_empty());
    }
}
