use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, code_name, finding, located, missing, numbered, FileFinding, ParseError};

const TOOL: &str = "elvis";

/// What elvis writes to stderr when `rock` finds anything.
const FAILED: &str = "Elvis: linting failed";

/// The prefix of a line elvis prints when the run itself fails.
const ERROR: &str = "Error: ";

/// Findings from `elvis --output-format parsable rock`: one `File:Line:Rule:Message` per line, on exit 0 or 1.
pub fn parse_elvis(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}{}", code_name(code), out.trim(), err.trim()),
        ));
    }
    let mut findings = Vec::new();
    let mut errors = Vec::new();
    for line in out.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(error) = line.strip_prefix(ERROR) {
            errors.push(error.trim().to_owned());
            continue;
        }
        findings.push(located_finding(line, files)?);
    }
    for line in err.lines().map(str::trim) {
        if line.is_empty() || line == FAILED {
            continue;
        }
        errors.push(line.to_owned());
    }
    if !errors.is_empty() {
        return Err(ParseError::shape(TOOL, errors.join("; ")));
    }
    if code == Some(1) && findings.is_empty() {
        return Err(ParseError::shape(TOOL, "exit 1 with no findings"));
    }
    findings.sort_by(|a, b| {
        (
            &a.file,
            a.finding.start.line,
            &a.finding.rule_id,
            a.finding.start.column,
        )
            .cmp(&(
                &b.file,
                b.finding.start.line,
                &b.finding.rule_id,
                b.finding.start.column,
            ))
    });
    Ok(findings)
}

fn located_finding(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let (file, rest) = located(TOOL, line, files)?;
    let mut parts = rest.splitn(3, ':');
    let row = numbered(TOOL, "line", line, parts.next())?.max(1);
    let rule = parts.next().unwrap_or_default();
    if rule.is_empty()
        || !rule
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(missing(TOOL, "rule", line));
    }
    let message = parts.next().unwrap_or_default().trim();
    if message.is_empty() {
        return Err(missing(TOOL, "message", line));
    }
    let (column, message) = positioned(row, message);
    Ok(finding(
        TOOL,
        file,
        rule.to_owned(),
        message.to_owned(),
        ToolSeverity::Warning,
        TextPosition { line: row, column },
        ColumnEncoding::Scalars,
    ))
}

/// The column a message spells in its `At line L, column C, ` lead, and the message without it.
fn positioned(row: u64, message: &str) -> (u64, &str) {
    let lead = format!("At line {row}, ");
    let Some(tail) = message.strip_prefix(lead.as_str()) else {
        return (1, message);
    };
    let Some(after) = tail.strip_prefix("column ") else {
        return (1, tail);
    };
    match after.split_once(", ") {
        Some((column, rest)) if !rest.is_empty() => match column.parse::<u64>() {
            Ok(column) => (column.max(1), rest),
            Err(_) => (1, tail),
        },
        _ => (1, tail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "src/sub/dirty.erl:5:no_debug_call:At line 5, column 12, an unexpected debug call to 'io:format/2' was found.\nsrc/sub/dirty.erl:5:function_naming_convention:At line 5, column 1, the name of function \"myFun\" is not acceptable by regular expression '^[a-z](_?[a-z0-9]+)*(_test_)?$'.\n";

    #[test]
    fn elvis_reports_findings_sorted_with_their_column() {
        let files = ["src/sub/dirty.erl", "src/clean.erl"];
        let findings =
            parse_elvis(DIRTY.as_bytes(), FAILED.as_bytes(), Some(1), &files).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.rule_id, "function_naming_convention");
        assert_eq!(findings[1].finding.rule_id, "no_debug_call");
        assert_eq!(
            findings[1].finding.start,
            TextPosition {
                line: 5,
                column: 12
            }
        );
        assert_eq!(
            findings[1].finding.message,
            "an unexpected debug call to 'io:format/2' was found."
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.tool_id, "elvis");
    }

    #[test]
    fn elvis_accepts_findings_on_a_clean_exit_and_a_quiet_clean_run() {
        let files = ["src/sub/dirty.erl"];
        assert_eq!(
            parse_elvis(DIRTY.as_bytes(), b"", Some(0), &files)
                .expect("parsed")
                .len(),
            2
        );
        assert!(parse_elvis(b"", b"", Some(0), &files)
            .expect("parsed")
            .is_empty());
        assert!(parse_elvis(b"\n", b"\n", Some(0), &files)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn elvis_keeps_messages_without_a_position_lead() {
        let files = ["a.erl"];
        let plain = parse_elvis(
            b"a.erl:3:max_module_length:module too long\n",
            b"",
            Some(1),
            &files,
        )
        .expect("parsed");
        assert_eq!(plain[0].finding.start.column, 1);
        assert_eq!(plain[0].finding.message, "module too long");
        let lined = parse_elvis(
            b"a.erl:3:line_length:At line 3, it is long\n",
            b"",
            Some(1),
            &files,
        )
        .expect("parsed");
        assert_eq!(lined[0].finding.message, "it is long");
        let odd = parse_elvis(
            b"a.erl:3:r:At line 3, column x, odd\n",
            b"",
            Some(1),
            &files,
        )
        .expect("parsed");
        assert_eq!(odd[0].finding.message, "column x, odd");
        let bare =
            parse_elvis(b"a.erl:3:r:At line 3, column 4\n", b"", Some(1), &files).expect("parsed");
        assert_eq!(bare[0].finding.message, "column 4");
        let other_line = parse_elvis(
            b"a.erl:0:r:At line 9, column 4, far\n",
            b"",
            Some(1),
            &files,
        )
        .expect("parsed");
        assert_eq!(other_line[0].finding.start.line, 1);
        assert_eq!(other_line[0].finding.message, "At line 9, column 4, far");
    }

    #[test]
    fn elvis_rejects_output_outside_the_grammar() {
        let files = ["a.erl"];
        assert!(matches!(
            parse_elvis(b"b.erl:1:r:m\n", b"", Some(1), &files),
            Err(ParseError::UnknownFile { path, .. }) if path == "b.erl"
        ));
        assert!(parse_elvis(b"something else\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:x:r:m\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:Bad Rule:m\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1::m\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:r: \n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:r\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"", FAILED.as_bytes(), Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:r:m\n", b"", Some(2), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:r:m\n", b"", None, &files).is_err());
        assert!(parse_elvis(&[0xff], b"", Some(0), &files).is_err());
        assert!(parse_elvis(b"", &[0xff], Some(0), &files).is_err());
    }

    #[test]
    fn elvis_fails_on_error_lines_and_unexpected_stderr() {
        let files = ["a.erl"];
        let config = b"Error: in file 'dx.config', key 'config', at list position number 1, yielded no files to analyse in [\"src/*.erl\"].\n";
        let err = parse_elvis(config, FAILED.as_bytes(), Some(1), &files).expect_err("error");
        assert!(err.to_string().contains("yielded no files"), "{err}");
        assert!(parse_elvis(b"Error: {enoent,\"a.erl\"}.\n", b"", Some(1), &files).is_err());
        assert!(parse_elvis(b"a.erl:1:r:m\n", b"escript: crash\n", Some(1), &files).is_err());
    }
}
