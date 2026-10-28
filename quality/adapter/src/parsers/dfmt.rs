use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, finding, unformatted_at, FileFinding, ParseError};

const TOOL: &str = "dfmt";

/// Findings from one `dfmt <file>` run, which writes the formatted text to stdout.
///
/// dfmt has no check mode and reports a parse error as `file(line:col)[error]: message` on stderr
/// while still exiting 0, so a run that printed an error is never judged by its stdout. Columns
/// are byte offsets. Any stderr line outside that grammar, an exit other than 0 without a parsed
/// error, or empty output for non-empty input is an error, never a clean file. `[warn]` lines are
/// accepted and ignored.
pub fn parse_dfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    as_text(TOOL, input)?;
    let errors = diagnostics(stderr, file)?;
    if !errors.is_empty() {
        if !matches!(code, Some(0) | Some(1)) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("exit {} with syntax errors (want 0 or 1)", code_name(code)),
            });
        }
        return Ok(errors);
    }
    if code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0)", code_name(code)),
        });
    }
    as_text(TOOL, stdout)?;
    if stdout.is_empty() && !input.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "no output for a non-empty file".to_owned(),
        });
    }
    if stdout == input {
        return Ok(Vec::new());
    }
    Ok(vec![unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Utf8)])
}

fn diagnostics(stderr: &[u8], file: &str) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let shape = |detail: &str| ParseError::Shape {
            tool: TOOL,
            detail: format!("{detail}: {line}"),
        };
        let rest = line
            .strip_prefix(file)
            .and_then(|rest| rest.strip_prefix('('))
            .ok_or_else(|| shape("unexpected stderr line"))?;
        let (position, rest) = rest
            .split_once(')')
            .ok_or_else(|| shape("unexpected stderr line"))?;
        let (row, col) = position
            .split_once(':')
            .ok_or_else(|| shape("bad position"))?;
        let row: u64 = row.parse().map_err(|_| shape("bad line"))?;
        let col: u64 = col.parse().map_err(|_| shape("bad column"))?;
        if row == 0 || col == 0 {
            return Err(shape("position is not 1-based"));
        }
        if rest.strip_prefix("[warn]: ").is_some() {
            continue;
        }
        let message = rest
            .strip_prefix("[error]: ")
            .ok_or_else(|| shape("unknown severity"))?;
        out.push(finding(
            TOOL,
            file,
            "syntax".to_owned(),
            message.trim().to_owned(),
            ToolSeverity::Error,
            TextPosition {
                line: row,
                column: col,
            },
            ColumnEncoding::Utf8,
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/a.d";
    const FORMATTED: &[u8] = b"void main()\n{\n}\n";
    const DIRTY: &[u8] = b"void main(){\n}\n";

    fn run(
        stdout: &[u8],
        stderr: &[u8],
        code: i32,
        input: &[u8],
    ) -> Result<Vec<FileFinding>, ParseError> {
        parse_dfmt(stdout, stderr, Some(code), input, FILE)
    }

    #[test]
    fn dfmt_is_clean_when_output_matches_input() {
        assert!(run(FORMATTED, b"", 0, FORMATTED)
            .expect("parsed")
            .is_empty());
        assert!(run(b"", b"", 0, b"").expect("parsed").is_empty());
    }

    #[test]
    fn dfmt_reports_a_file_whose_formatted_output_differs() {
        let findings = run(FORMATTED, b"", 0, DIRTY).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn dfmt_reports_a_parse_error_even_though_it_exits_zero() {
        let stderr = b"/s/a.d(2:5)[error]: Primary expression expected\n/s/a.d(2:8)[warn]: Empty declaration\n";
        let findings = run(DIRTY, stderr, 0, DIRTY).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 5);
    }

    #[test]
    fn dfmt_reports_a_lexer_error_that_exits_one() {
        let stderr = b"/s/a.d(1:2)[error]: `(` expected\n";
        assert_eq!(run(b"", stderr, 1, DIRTY).expect("parsed").len(), 1);
        assert!(run(b"", stderr, 2, DIRTY).is_err());
    }

    #[test]
    fn dfmt_fails_closed_on_anything_it_does_not_know() {
        assert!(run(FORMATTED, b"boom\n", 0, FORMATTED).is_err());
        assert!(run(FORMATTED, b"/s/b.d(1:1)[error]: x\n", 0, FORMATTED).is_err());
        assert!(run(FORMATTED, b"/s/a.d(1:1)[info]: x\n", 0, FORMATTED).is_err());
        assert!(run(FORMATTED, b"/s/a.d(0:1)[error]: x\n", 0, FORMATTED).is_err());
        assert!(run(FORMATTED, b"", 1, FORMATTED).is_err());
        assert!(parse_dfmt(FORMATTED, b"", None, FORMATTED, FILE).is_err());
        assert!(run(b"", b"", 0, DIRTY).is_err());
        assert!(run(FORMATTED, b"", 0, &[0xff]).is_err());
        assert!(run(&[0xff], b"", 0, FORMATTED).is_err());
        assert!(run(FORMATTED, &[0xff], 0, FORMATTED).is_err());
    }
}
