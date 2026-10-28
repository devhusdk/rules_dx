use serde::Deserialize;

use super::{check_output_size, code_name, known_native, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "selene";

#[derive(Debug, Deserialize)]
struct Span {
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

#[derive(Debug, Deserialize)]
struct Label {
    filename: String,
    span: Span,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    severity: String,
    code: String,
    message: String,
    primary_label: Label,
}

/// One `--display-style=json2` line. `Summary` is ignored; the exit code carries the verdict.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Line {
    Diagnostic(Box<Diagnostic>),
    Summary {},
}

fn severity(value: &str) -> Result<ToolSeverity, ParseError> {
    match value {
        "Error" => Ok(ToolSeverity::Error),
        "Warning" => Ok(ToolSeverity::Warning),
        "Note" | "Help" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unknown severity: {value}"),
        )),
    }
}

/// Selene spans are 0-based lines and character columns.
fn position(line: u64, column: u64) -> Result<TextPosition, ParseError> {
    match (line.checked_add(1), column.checked_add(1)) {
        (Some(line), Some(column)) => Ok(TextPosition { line, column }),
        _ => Err(ParseError::shape(TOOL, "position overflow")),
    }
}

/// `selene --display-style=json2` prints one JSON object per line. Exit 0 means clean and no
/// diagnostics; exit 1 with at least one diagnostic means findings (warnings also exit 1). Exit 1
/// with no diagnostics is a tool failure (missing file, bad std or config) and is an error, as is
/// any other exit, any non-JSON line and any finding for a file that was not passed. A parse
/// error is a diagnostic with code `parse_error`, so it is a finding.
pub fn parse_selene(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let parsed: Line = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        let Line::Diagnostic(diagnostic) = parsed else {
            continue;
        };
        let checked = known_native(TOOL, files, &diagnostic.primary_label.filename)?;
        let span = &diagnostic.primary_label.span;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                severity: severity(&diagnostic.severity)?,
                start: position(span.start_line, span.start_column)?,
                end: Some(position(span.end_line, span.end_column)?),
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} diagnostics", code_name(code), findings.len()),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/lua/a.lua", "/s/lua/b.lua"];

    const DIRTY: &str = r#"{"type":"Diagnostic","severity":"Warning","code":"undefined_variable","message":"`foo` is not defined","primary_label":{"filename":"/s/lua/a.lua","span":{"start":0,"start_line":2,"start_column":4,"end":3,"end_line":2,"end_column":7},"message":""},"notes":[],"secondary_labels":[]}
{"type":"Summary","errors":0,"warnings":1,"parse_errors":0}
"#;

    const SYNTAX: &str = r#"{"type":"Diagnostic","severity":"Error","code":"parse_error","message":"unexpected token `=`","primary_label":{"filename":"/s/lua/b.lua","span":{"start":6,"start_line":0,"start_column":6,"end":7,"end_line":0,"end_column":7},"message":"x"},"notes":[],"secondary_labels":[]}
"#;

    #[test]
    fn selene_is_clean_on_no_output_and_exit_zero() {
        assert!(parse_selene(b"", Some(0), &FILES).expect("parsed").is_empty());
        let summary = br#"{"type":"Summary","errors":0,"warnings":0,"parse_errors":0}"#;
        assert!(parse_selene(summary, Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn selene_converts_zero_based_positions() {
        let findings = parse_selene(DIRTY.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/lua/a.lua");
        assert_eq!(findings[0].finding.tool_id, "selene");
        assert_eq!(findings[0].finding.rule_id, "undefined_variable");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 3, column: 5 },
                Some(TextPosition { line: 3, column: 8 })
            )
        );
    }

    #[test]
    fn selene_reports_a_parse_error_as_a_finding() {
        let findings = parse_selene(SYNTAX.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/lua/b.lua");
        assert_eq!(findings[0].finding.rule_id, "parse_error");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn selene_fails_closed() {
        // Exit 1 without a diagnostic is a tool failure, never clean.
        assert!(parse_selene(b"", Some(1), &FILES).is_err());
        assert!(parse_selene(b"ERROR: no such file", Some(1), &FILES).is_err());
        // Diagnostics with exit 0.
        assert!(parse_selene(DIRTY.as_bytes(), Some(0), &FILES).is_err());
        // Other exits and signals.
        assert!(parse_selene(b"", Some(2), &FILES).is_err());
        assert!(parse_selene(DIRTY.as_bytes(), Some(101), &FILES).is_err());
        assert!(parse_selene(b"", None, &FILES).is_err());
        // Garbage, truncation and non-UTF-8.
        assert!(parse_selene(&DIRTY.as_bytes()[..60], Some(1), &FILES).is_err());
        assert!(parse_selene(&[0xff, 0xfe], Some(1), &FILES).is_err());
        // A finding in a file that was not passed.
        assert!(parse_selene(DIRTY.as_bytes(), Some(1), &["/s/other.lua"]).is_err());
        // An unknown severity.
        let odd = DIRTY.replace("\"Warning\"", "\"Fatal\"");
        assert!(parse_selene(odd.as_bytes(), Some(1), &FILES).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn selene_accepts_either_separator_on_windows() {
        let files = ["C:\\s\\lua\\a.lua"];
        let report = DIRTY.replace("/s/lua/a.lua", "C:/s/lua/a.lua");
        assert!(parse_selene(report.as_bytes(), Some(1), &files).is_ok());
    }
}
