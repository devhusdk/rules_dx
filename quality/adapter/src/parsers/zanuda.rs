use serde::Deserialize;

use super::{check_output_size, code_name, known_native, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "zanuda";

#[derive(Debug, Deserialize)]
struct Start {
    line: u64,
}

#[derive(Debug, Deserialize)]
struct Range {
    start: Start,
}

#[derive(Debug, Deserialize)]
struct Location {
    path: String,
    range: Range,
}

#[derive(Debug, Deserialize)]
struct Code {
    value: String,
}

#[derive(Debug, Deserialize)]
struct Line {
    message: String,
    location: Location,
    code: Code,
}

/// zanuda reports the line only, so the finding starts at column 1 and has no end. The one
/// place that builds a position, so a tool that later reports columns changes only this.
fn position(line: u64) -> Result<TextPosition, ParseError> {
    if line == 0 {
        return Err(ParseError::shape(TOOL, "line 0 in a finding"));
    }
    Ok(TextPosition { line, column: 1 })
}

/// Parses the `-ordjsonl` file of `zanuda -cmt-list`. One JSON object per line. zanuda exits 0
/// whether or not it found anything, and 1 when it skipped or could not read a listed entry, so
/// any exit other than 0 is an error, as is a finding with no rule id, a line below 1, a
/// malformed line and a finding for a file that was not listed. zanuda prints every finding as
/// `INFO`; the level carries no information, so each finding is a warning.
pub fn parse_zanuda(
    report: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, report)?;
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let text = std::str::from_utf8(report).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let parsed: Line = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        if parsed.code.value.is_empty() {
            return Err(ParseError::shape(TOOL, "finding with an empty rule id"));
        }
        let checked = known_native(TOOL, files, &parsed.location.path)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: parsed.code.value,
                message: parsed.message.trim_end().to_owned(),
                severity: ToolSeverity::Warning,
                start: position(parsed.location.range.start.line)?,
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::super::MAX_OUTPUT_BYTES;
    use super::*;

    const FILES: [&str; 2] = ["/s/a.ml", "/s/b.mli"];

    const DIRTY: &str = concat!(
        r#"{"message":"Executing 'if ... then true' smells bad\n","location":{"path":"/s/a.ml","range":{"start":{"line":1,"column":1}}},"severity":"INFO","code":{"value":"if_bool","url":"https://kakadu.github.io/zanuda/"}}"#,
        "\n",
        r#"{"message":"Type name `fooBar` should be in snake case","location":{"path":"/s/b.mli","range":{"start":{"line":4,"column":1}}},"severity":"INFO","code":{"value":"camel_cased_types","url":"https://kakadu.github.io/zanuda/"}}"#,
        "\n",
    );

    #[test]
    fn zanuda_is_clean_on_an_empty_report() {
        assert!(parse_zanuda(b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn zanuda_reads_the_rule_line_and_file() {
        let findings = parse_zanuda(DIRTY.as_bytes(), Some(0), &FILES).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.ml");
        assert_eq!(findings[0].finding.tool_id, "zanuda");
        assert_eq!(findings[0].finding.rule_id, "if_bool");
        assert_eq!(
            findings[0].finding.message,
            "Executing 'if ... then true' smells bad"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 1, column: 1 }
        );
        assert_eq!(findings[0].finding.end, None);
        assert_eq!(findings[1].file, "/s/b.mli");
        assert_eq!(findings[1].finding.start.line, 4);
    }

    #[test]
    fn zanuda_accepts_crlf_and_a_missing_final_newline() {
        let crlf = DIRTY.replace('\n', "\r\n");
        assert_eq!(
            parse_zanuda(crlf.as_bytes(), Some(0), &FILES)
                .expect("parsed")
                .len(),
            2
        );
        assert_eq!(
            parse_zanuda(DIRTY.trim_end().as_bytes(), Some(0), &FILES)
                .expect("parsed")
                .len(),
            2
        );
    }

    #[test]
    fn zanuda_fails_closed() {
        // Exit 1 means a listed entry was skipped or unreadable; the report is incomplete.
        assert!(parse_zanuda(b"", Some(1), &FILES).is_err());
        assert!(parse_zanuda(DIRTY.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_zanuda(b"", Some(2), &FILES).is_err());
        assert!(parse_zanuda(b"", None, &FILES).is_err());
        // Garbage, truncation and non-UTF-8.
        assert!(parse_zanuda(b"not json", Some(0), &FILES).is_err());
        assert!(parse_zanuda(&DIRTY.as_bytes()[..60], Some(0), &FILES).is_err());
        assert!(parse_zanuda(&[0xff, 0xfe], Some(0), &FILES).is_err());
        // A finding in a file that was not listed.
        assert!(parse_zanuda(DIRTY.as_bytes(), Some(0), &["/s/other.ml"]).is_err());
        // A finding without a rule id.
        let no_code = DIRTY.replace(
            r#""code":{"value":"if_bool","url":"https://kakadu.github.io/zanuda/"}"#,
            r#""code":{}"#,
        );
        assert!(parse_zanuda(no_code.as_bytes(), Some(0), &FILES).is_err());
        let empty_code = DIRTY.replace("\"if_bool\"", "\"\"");
        assert!(parse_zanuda(empty_code.as_bytes(), Some(0), &FILES).is_err());
        // Line 0.
        let line_zero = DIRTY.replace("\"line\":1", "\"line\":0");
        assert!(parse_zanuda(line_zero.as_bytes(), Some(0), &FILES).is_err());
    }

    #[test]
    fn zanuda_rejects_oversize_output() {
        let big = vec![b' '; MAX_OUTPUT_BYTES + 1];
        assert!(parse_zanuda(&big, Some(0), &FILES).is_err());
    }
}
