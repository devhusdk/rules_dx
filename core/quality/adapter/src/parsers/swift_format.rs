use super::gnu_diagnostics::{Dialect, Disposition, read};
use super::{FileFinding, ParseError, as_text, code_name, rewrite_format};
use crate::{ColumnEncoding, ToolSeverity};

const TOOL: &str = "swift_format";

const SYNTAX: Dialect = Dialect {
    tool: TOOL,
    encoding: ColumnEncoding::Utf8,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        _ => None,
    },
    bracket_rule: false,
};

/// Findings from one `swift-format format <file>` run, which writes the formatted text to stdout.
///
/// Exit 0 means the file parsed, so stdout is compared with the input and a difference is one
/// finding. Exit 1 means a syntax error: each `file:LINE:COL: error: message` stderr line becomes
/// one syntax finding. Any other exit, or exit 1 without such a line, is an error.
pub fn parse_swift_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    as_text(TOOL, input)?;
    as_text(TOOL, stderr)?;
    match code {
        Some(0) => rewrite_format(TOOL, stdout, Some(0), input, file, ColumnEncoding::Utf8),
        Some(1) => syntax_errors(stderr, code, file),
        other => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0 or 1)", code_name(other)),
        }),
    }
}

fn syntax_errors(
    stderr: &[u8],
    code: Option<i32>,
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    let mut found = read(&SYNTAX, stderr, code, &[file])?;
    for finding in &mut found {
        finding.finding.rule_id = "syntax".to_owned();
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/a.swift";
    const TIDY: &[u8] = b"func f() {}\n";
    const UNTIDY: &[u8] = b"func  f() {}\n";
    const BROKEN_STDERR: &[u8] = b"/s/a.swift:1:6: error: expected identifier in function\n";

    fn run(
        stdout: &[u8],
        stderr: &[u8],
        code: i32,
        input: &[u8],
    ) -> Result<Vec<FileFinding>, ParseError> {
        parse_swift_format(stdout, stderr, Some(code), input, FILE)
    }

    #[test]
    fn swift_format_is_clean_when_output_matches_input() {
        assert!(run(TIDY, b"", 0, TIDY).expect("parsed").is_empty());
    }

    #[test]
    fn swift_format_reports_a_file_whose_formatted_output_differs() {
        let findings = run(TIDY, b"", 0, UNTIDY).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn swift_format_reports_a_syntax_error_at_its_position() {
        let findings = run(b"", BROKEN_STDERR, 1, UNTIDY).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 6);
        assert_eq!(
            findings[0].finding.message,
            "expected identifier in function"
        );
    }

    #[test]
    fn swift_format_fails_closed_on_anything_it_does_not_know() {
        assert!(run(b"", b"boom\n", 1, UNTIDY).is_err());
        assert!(run(TIDY, b"", 2, TIDY).is_err());
        assert!(parse_swift_format(TIDY, b"", None, TIDY, FILE).is_err());
        assert!(run(b"", b"", 0, UNTIDY).is_err());
        assert!(run(TIDY, b"", 0, &[0xff]).is_err());
        assert!(run(TIDY, &[0xff], 0, TIDY).is_err());
    }
}
