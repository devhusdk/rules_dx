use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, finding, unformatted_at, FileFinding, ParseError};

const TOOL: &str = "perltidy";

/// Findings from one `perltidy --standard-output <file>` run.
///
/// Exit 0 or 1 means perltidy formatted the file, so stdout is compared with the input and a
/// difference is one finding. Exit 2 means the source did not parse: perltidy then echoes the
/// input and describes the problem on stderr as `file:LINE: message`, which becomes one syntax
/// finding at the first such line (line 1 when none parses). Any other exit, or empty output for
/// non-empty input, is an error, never a clean file.
pub fn parse_perltidy(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    as_text(TOOL, input)?;
    let stderr = as_text(TOOL, stderr)?;
    match code {
        Some(2) => Ok(vec![syntax_finding(stderr, file)]),
        Some(0) | Some(1) => {
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
        other => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0, 1 or 2)", code_name(other)),
        }),
    }
}

fn syntax_finding(stderr: &str, file: &str) -> FileFinding {
    let mut first = None;
    for line in stderr.lines() {
        let Some(rest) = line
            .strip_prefix(file)
            .and_then(|rest| rest.strip_prefix(':'))
        else {
            continue;
        };
        let Some((row, message)) = rest.split_once(": ") else {
            continue;
        };
        if let Ok(row) = row.parse::<u64>() {
            if row > 0 {
                first = Some((row, message.trim().to_owned()));
                break;
            }
        }
    }
    let (row, message) =
        first.unwrap_or_else(|| (1, "perltidy could not parse the file".to_owned()));
    finding(
        TOOL,
        file,
        "syntax".to_owned(),
        message,
        ToolSeverity::Error,
        TextPosition {
            line: row,
            column: 1,
        },
        ColumnEncoding::Utf8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/a.pm";
    const TIDY: &[u8] = b"sub f {\n    return 1;\n}\n1;\n";
    const UNTIDY: &[u8] = b"sub  f{\nreturn   1}\n1;\n";
    const BROKEN: &[u8] = b"sub f { my $x = (1;\n";
    const BROKEN_STDERR: &[u8] = b"/s/a.pm: Begin Error Output Stream\n/s/a.pm:1: final indentation level: 2\n/s/a.pm:1: Formatting will be skipped because level error '2' exceeds -maxle=1; use -maxle=-1 to force formatting\n/s/a.pm: \n/s/a.pm: Final nesting depth of '{'s is 1\n/s/a.pm:1: To save a full .LOG file rerun with -g\n";

    fn run(
        stdout: &[u8],
        stderr: &[u8],
        code: i32,
        input: &[u8],
    ) -> Result<Vec<FileFinding>, ParseError> {
        parse_perltidy(stdout, stderr, Some(code), input, FILE)
    }

    #[test]
    fn perltidy_is_clean_when_output_matches_input() {
        assert!(run(TIDY, b"", 0, TIDY).expect("parsed").is_empty());
        assert!(run(b"", b"", 0, b"").expect("parsed").is_empty());
    }

    #[test]
    fn perltidy_reports_a_file_whose_formatted_output_differs() {
        let findings = run(TIDY, b"", 0, UNTIDY).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn perltidy_reports_a_syntax_error_at_the_first_numbered_stderr_line() {
        let findings = run(BROKEN, BROKEN_STDERR, 2, BROKEN).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "syntax");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.message, "final indentation level: 2");
    }

    #[test]
    fn perltidy_reports_a_syntax_error_with_unparseable_stderr_at_line_one() {
        let findings = run(BROKEN, b"boom\n", 2, BROKEN).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 1);
    }

    #[test]
    fn perltidy_fails_closed_on_anything_it_does_not_know() {
        assert!(run(TIDY, b"", 3, TIDY).is_err());
        assert!(parse_perltidy(TIDY, b"", None, TIDY, FILE).is_err());
        assert!(run(b"", b"", 0, UNTIDY).is_err());
        assert!(run(TIDY, b"", 0, &[0xff]).is_err());
        assert!(run(&[0xff], b"", 0, TIDY).is_err());
        assert!(run(TIDY, &[0xff], 0, TIDY).is_err());
    }
}
