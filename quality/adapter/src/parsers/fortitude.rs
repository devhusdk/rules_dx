use serde::Deserialize;

use super::{check_output_size, code_name, known_native, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "fortitude";

#[derive(Debug, Deserialize)]
struct Position {
    column: u64,
    row: u64,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    code: String,
    filename: String,
    location: Position,
    end_location: Position,
    message: String,
    severity: String,
}

fn severity(value: &str) -> Result<ToolSeverity, ParseError> {
    match value {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        "info" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unknown severity: {value}"),
        )),
    }
}

fn position(what: &str, at: &Position) -> Result<TextPosition, ParseError> {
    if at.row == 0 || at.column == 0 {
        return Err(ParseError::shape(TOOL, format!("zero {what} position")));
    }
    Ok(TextPosition {
        line: at.row,
        column: at.column,
    })
}

/// `fortitude check --output-format json` prints one JSON array on stdout: `[]` and exit 0 when
/// every file is clean, a non-empty array and exit 1 otherwise. Syntax errors (`E001`) and
/// unreadable files (`E000`) are diagnostics in the same array, so they are findings, not
/// failures to run. Any other exit (2 is a usage error) or an exit that disagrees with the array
/// is an error here and never a clean result.
pub fn parse_fortitude(
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
    let diagnostics: Vec<Diagnostic> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    if diagnostics.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} with {} diagnostics",
                code_name(code),
                diagnostics.len()
            ),
        ));
    }
    let mut findings = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        let checked = known_native(TOOL, files, &diagnostic.filename)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                severity: severity(&diagnostic.severity)?,
                start: position("start", &diagnostic.location)?,
                end: Some(position("end", &diagnostic.end_location)?),
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
    use super::*;

    const FILES: [&str; 2] = ["/s/fortran/a.f90", "/s/fortran/b.F90"];

    const DIRTY: &str = r#"[{"code":"C001","name":"implicit-typing","severity":"error","end_location":{"column":1,"row":2},"filename":"/s/fortran/a.f90","fix":{"applicability":"unsafe","edits":[],"message":"Insert `implicit none`"},"location":{"column":1,"row":1},"message":"program uses implicit typing","url":"https://fortitude.readthedocs.io//en/stable/rules/implicit-typing"}]"#;

    const SYNTAX: &str = r#"[{"code":"E001","name":"syntax-error","severity":"error","end_location":{"column":1,"row":5},"filename":"/s/fortran/b.F90","fix":null,"location":{"column":1,"row":1},"message":"Syntax error","url":null}]"#;

    #[test]
    fn fortitude_is_clean_on_an_empty_array_and_exit_zero() {
        assert!(parse_fortitude(b"[]", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn fortitude_locates_each_diagnostic() {
        let findings = parse_fortitude(DIRTY.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/fortran/a.f90");
        assert_eq!(findings[0].finding.tool_id, "fortitude");
        assert_eq!(findings[0].finding.rule_id, "C001");
        assert_eq!(findings[0].finding.message, "program uses implicit typing");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 1, column: 1 },
                Some(TextPosition { line: 2, column: 1 })
            )
        );
    }

    #[test]
    fn fortitude_reports_a_syntax_error_as_a_finding() {
        let findings = parse_fortitude(SYNTAX.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/fortran/b.F90");
        assert_eq!(findings[0].finding.rule_id, "E001");
    }

    #[test]
    fn fortitude_fails_closed() {
        // Exit and array disagree.
        assert!(parse_fortitude(b"[]", Some(1), &FILES).is_err());
        assert!(parse_fortitude(DIRTY.as_bytes(), Some(0), &FILES).is_err());
        // Unknown exits, including a usage error and a signal.
        assert!(parse_fortitude(b"[]", Some(2), &FILES).is_err());
        assert!(parse_fortitude(DIRTY.as_bytes(), Some(2), &FILES).is_err());
        assert!(parse_fortitude(b"[]", None, &FILES).is_err());
        // A truncated or garbage report.
        assert!(parse_fortitude(&DIRTY.as_bytes()[..60], Some(1), &FILES).is_err());
        assert!(parse_fortitude(b"error: bad flag", Some(1), &FILES).is_err());
        assert!(parse_fortitude(b"", Some(0), &FILES).is_err());
        assert!(parse_fortitude(&[0xff, 0xfe], Some(1), &FILES).is_err());
        // A finding in a file that was not passed.
        assert!(parse_fortitude(DIRTY.as_bytes(), Some(1), &["/s/other.f90"]).is_err());
        // A zero position, or a severity this parser does not know.
        let zero = DIRTY.replace(r#""column":1,"row":1"#, r#""column":0,"row":1"#);
        assert!(parse_fortitude(zero.as_bytes(), Some(1), &FILES).is_err());
        let odd = DIRTY.replace("\"error\"", "\"fatal\"");
        assert!(parse_fortitude(odd.as_bytes(), Some(1), &FILES).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn fortitude_accepts_either_separator_on_windows() {
        let files = ["C:\\s\\fortran\\a.f90"];
        let report = DIRTY.replace("/s/fortran/a.f90", "C:/s/fortran/a.f90");
        assert!(parse_fortitude(report.as_bytes(), Some(1), &files).is_ok());
    }
}
