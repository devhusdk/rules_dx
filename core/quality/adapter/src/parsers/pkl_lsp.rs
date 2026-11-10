use serde::Deserialize;

use super::{FileFinding, ParseError, check_output_size, code_name, known_native, lines};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "pkl_lsp";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    file: String,
    line: u64,
    col: u64,
    severity: u64,
    code: String,
    message: String,
}

fn severity(value: u64) -> Result<ToolSeverity, ParseError> {
    match value {
        1 => Ok(ToolSeverity::Error),
        2 => Ok(ToolSeverity::Warning),
        3 | 4 => Ok(ToolSeverity::Info),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unknown severity: {value}"),
        )),
    }
}

/// `lsp_check` prints one JSON object per diagnostic and exits 0 when there are none, 1 when
/// there are some. Any other exit, bad JSON, a file that was not passed, or an exit that
/// disagrees with the lines is an error here and never a clean result.
pub fn parse_pkl_lsp(
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
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let diagnostic: Diagnostic =
            serde_json::from_str(line).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
        if diagnostic.line == 0 || diagnostic.col == 0 {
            return Err(ParseError::shape(TOOL, "zero position"));
        }
        let checked = known_native(TOOL, files, &diagnostic.file)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.code,
                message: diagnostic.message,
                severity: severity(diagnostic.severity)?,
                start: TextPosition {
                    line: diagnostic.line,
                    column: diagnostic.col,
                },
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Utf16,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} with {} diagnostics",
                code_name(code),
                findings.len()
            ),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/pkl/a.pkl", "/s/pkl/b.pkl"];

    const DIRTY: &str = r#"{"file":"/s/pkl/a.pkl","line":3,"col":5,"severity":2,"code":"unused","message":"Unused local"}"#;

    #[test]
    fn pkl_lsp_is_clean_on_no_output_and_exit_zero() {
        assert!(
            parse_pkl_lsp(b"", Some(0), &FILES)
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn pkl_lsp_locates_each_diagnostic() {
        let two = format!(
            "{DIRTY}\n{}\n",
            DIRTY
                .replace("a.pkl", "b.pkl")
                .replace("\"severity\":2", "\"severity\":1")
        );
        let findings = parse_pkl_lsp(two.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/pkl/a.pkl");
        assert_eq!(findings[0].finding.tool_id, "pkl_lsp");
        assert_eq!(findings[0].finding.rule_id, "unused");
        assert_eq!(findings[0].finding.message, "Unused local");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 3, column: 5 }
        );
        assert_eq!(findings[1].file, "/s/pkl/b.pkl");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn pkl_lsp_fails_closed() {
        assert!(parse_pkl_lsp(b"", Some(1), &FILES).is_err());
        assert!(parse_pkl_lsp(DIRTY.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_pkl_lsp(b"", Some(2), &FILES).is_err());
        assert!(parse_pkl_lsp(DIRTY.as_bytes(), Some(2), &FILES).is_err());
        assert!(parse_pkl_lsp(b"", None, &FILES).is_err());
        assert!(parse_pkl_lsp(&DIRTY.as_bytes()[..40], Some(1), &FILES).is_err());
        assert!(parse_pkl_lsp(b"lsp_check: boom", Some(1), &FILES).is_err());
        assert!(parse_pkl_lsp(&[0xff, 0xfe], Some(1), &FILES).is_err());
        assert!(parse_pkl_lsp(DIRTY.as_bytes(), Some(1), &["/s/other.pkl"]).is_err());
        let zero = DIRTY.replace("\"col\":5", "\"col\":0");
        assert!(parse_pkl_lsp(zero.as_bytes(), Some(1), &FILES).is_err());
        let odd = DIRTY.replace("\"severity\":2", "\"severity\":9");
        assert!(parse_pkl_lsp(odd.as_bytes(), Some(1), &FILES).is_err());
        let extra = DIRTY.replace("}", ",\"x\":1}");
        assert!(parse_pkl_lsp(extra.as_bytes(), Some(1), &FILES).is_err());
    }
}
