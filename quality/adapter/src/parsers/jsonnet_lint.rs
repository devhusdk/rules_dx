use super::{as_text, code_name, finding, prefixed, row_col, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "jsonnet_lint";

/// The closing line `jsonnet-lint` prints after its last problem.
const TRAILER: &str = "Problems found!";

/// `jsonnet-lint` prints nothing and exits 0 when clean. Otherwise it exits 2 and writes every
/// problem to stderr as `<path>:<line>:<column>[-<column>] <message>`, each followed by the
/// offending source line, then `Problems found!`. Exit 1 is a failure to run (an unreadable file)
/// and carries no location, so it is an error here and never a clean result.
pub fn parse_jsonnet_lint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let printed = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    match code {
        Some(0) if printed.trim().is_empty() && errors.trim().is_empty() => Ok(Vec::new()),
        Some(2) if printed.trim().is_empty() => problems(errors, files),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} with output", code_name(code)),
        )),
    }
}

fn problems(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let lines: Vec<&str> = stderr.lines().map(str::trim_end).collect();
    if lines.iter().rev().find(|line| !line.is_empty()) != Some(&TRAILER) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit 2 without the `{TRAILER}` trailer"),
        ));
    }
    let mut findings = Vec::new();
    for line in lines {
        let located = prefixed(line, files).and_then(|(file, rest)| {
            let (location, message) = rest.split_once(' ')?;
            let (row, column) = start_position(location)?;
            Some((file, row, column, message))
        });
        // Anything else is the echoed source line or the trailer.
        let Some((file, row, column, message)) = located else {
            continue;
        };
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.trim().to_owned(),
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    if findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 2 with no located problems".to_owned(),
        ));
    }
    Ok(findings)
}

fn start_position(location: &str) -> Option<(u64, u64)> {
    let start = location
        .split_once(")-(")
        .map_or(location, |(start, _)| start);
    let start = start.strip_prefix('(').unwrap_or(start);
    row_col(start.split('-').next()?, ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/jsonnet/a.jsonnet", "/s/jsonnet/lib.libsonnet"];

    #[test]
    fn jsonnet_lint_is_clean_on_exit_zero_with_no_output() {
        assert!(parse_jsonnet_lint(b"", b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn jsonnet_lint_locates_each_problem() {
        let stderr = b"/s/jsonnet/a.jsonnet:1:7-17 Unused variable: unused\n\nlocal unused = 1;\n\n\n\
            /s/jsonnet/lib.libsonnet:3:19-26 Indexed object has no field \"foo\"\n\n{ b: std.foo }\n\n\nProblems found!\n";
        let findings = parse_jsonnet_lint(b"", stderr, Some(2), &FILES).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/jsonnet/a.jsonnet");
        assert_eq!(findings[0].finding.message, "Unused variable: unused");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 7);
        assert_eq!(findings[1].file, "/s/jsonnet/lib.libsonnet");
        assert_eq!(findings[1].finding.start.line, 3);
        assert_eq!(findings[1].finding.start.column, 19);
    }

    #[test]
    fn jsonnet_lint_locates_a_syntax_error_without_a_range() {
        let stderr = b"/s/jsonnet/a.jsonnet:2:1 Unexpected end of file\n\n\n\n\nProblems found!\n";
        let findings = parse_jsonnet_lint(b"", stderr, Some(2), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.message, "Unexpected end of file");
        assert_eq!(findings[0].finding.start.line, 2);
    }

    #[test]
    fn jsonnet_lint_fails_closed() {
        let located = b"/s/jsonnet/a.jsonnet:1:1 m\n\nProblems found!\n";
        // Output on a clean exit.
        assert!(parse_jsonnet_lint(b"", located, Some(0), &FILES).is_err());
        assert!(parse_jsonnet_lint(b"x", b"", Some(0), &FILES).is_err());
        // No output, or no located problem, on a failing exit.
        assert!(parse_jsonnet_lint(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_jsonnet_lint(b"", b"Problems found!\n", Some(2), &FILES).is_err());
        // A run failure carries no location.
        assert!(parse_jsonnet_lint(
            b"",
            b"ERROR: open a.jsonnet: no such file or directory\n",
            Some(1),
            &FILES
        )
        .is_err());
        // A problem in a file that was not passed.
        assert!(parse_jsonnet_lint(
            b"",
            b"/s/other.jsonnet:1:1 m\n\nProblems found!\n",
            Some(2),
            &FILES
        )
        .is_err());
        // A truncated run lacks the trailer.
        assert!(parse_jsonnet_lint(b"", b"/s/jsonnet/a.jsonnet:1:1 m\n", Some(2), &FILES).is_err());
        // Signal exit and other codes.
        assert!(parse_jsonnet_lint(b"", located, None, &FILES).is_err());
        assert!(parse_jsonnet_lint(b"", located, Some(3), &FILES).is_err());
        assert!(parse_jsonnet_lint(b"", &[0xff, 0xfe], Some(2), &FILES).is_err());
    }
}
