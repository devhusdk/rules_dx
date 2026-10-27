use super::{
    as_text, code_name, finding, listed_paths, prefixed, row_col, FileFinding, ParseError, Spelling,
};
use crate::ToolSeverity;

const TOOL: &str = "jsonnetfmt";

/// `--test` lists each file it would rewrite on stdout and exits 2, and reports the first
/// syntax error as `<path>:<line>:<column> <message>` on stderr with exit 1.
pub fn parse_jsonnetfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let listed = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    match code {
        Some(0) if listed.trim().is_empty() && errors.trim().is_empty() => Ok(Vec::new()),
        Some(1 | 2) => {
            let mut findings = Vec::new();
            if !listed.trim().is_empty() {
                findings.extend(listed_paths(
                    TOOL,
                    stdout,
                    code,
                    files,
                    &[],
                    Spelling::Exact,
                )?);
            }
            findings.extend(syntax_errors(errors, files)?);
            if findings.is_empty() {
                return Err(ParseError::shape(
                    TOOL,
                    format!(
                        "exit {} with no files and no located errors",
                        code_name(code)
                    ),
                ));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} with output", code_name(code)),
        )),
    }
}

fn syntax_errors(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in stderr.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        let located = prefixed(line, files).and_then(|(file, rest)| {
            let (location, message) = rest.split_once(' ')?;
            let (row, column) = start_position(location)?;
            Some((file, row, column, message))
        });
        let Some((file, row, column, message)) = located else {
            if let Some(last) = findings.last_mut() {
                last.finding.message.push('\n');
                last.finding.message.push_str(line);
                continue;
            }
            return Err(ParseError::shape(
                TOOL,
                format!("error without a location: {line}"),
            ));
        };
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.trim().to_owned(),
            ToolSeverity::Error,
            row,
            column,
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

    const FILES: [&str; 2] = ["/s/jsonnet/Sample.jsonnet", "/s/jsonnet/lib.libsonnet"];

    #[test]
    fn jsonnetfmt_reports_listed_files() {
        let findings =
            parse_jsonnetfmt(b"/s/jsonnet/Sample.jsonnet\n", b"", Some(2), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/jsonnet/Sample.jsonnet");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_jsonnetfmt(b"", b"", Some(0), &FILES).expect("parsed");
        assert!(clean.is_empty());
    }

    #[test]
    fn jsonnetfmt_locates_a_syntax_error() {
        let findings = parse_jsonnetfmt(
            b"",
            b"/s/jsonnet/lib.libsonnet:2:1 Unexpected end of file\n",
            Some(1),
            &FILES,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/jsonnet/lib.libsonnet");
        assert_eq!(findings[0].finding.message, "Unexpected end of file");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
    }

    #[test]
    fn jsonnetfmt_locates_a_multi_line_range_with_a_multi_line_message() {
        let findings = parse_jsonnetfmt(
            b"",
            b"/s/jsonnet/Sample.jsonnet:(3:3)-(4:3) Expected a comma before next field\n\
              /s/jsonnet/lib.libsonnet:(1:13)-(3:4) Expected , or ; but got (STRING_BLOCK, \"a\n\")\n",
            Some(1),
            &FILES,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/jsonnet/Sample.jsonnet");
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 3);
        assert_eq!(findings[1].file, "/s/jsonnet/lib.libsonnet");
        assert_eq!(findings[1].finding.start.line, 1);
        assert_eq!(findings[1].finding.start.column, 13);
        assert!(findings[1].finding.message.ends_with("\"a\n\")"));
    }

    #[test]
    fn jsonnetfmt_keeps_listed_files_beside_an_error() {
        let findings = parse_jsonnetfmt(
            b"/s/jsonnet/Sample.jsonnet\n",
            b"/s/jsonnet/lib.libsonnet:3:5-8 Unexpected token\n",
            Some(1),
            &FILES,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[1].finding.start.column, 5);
    }

    #[test]
    fn jsonnetfmt_never_reads_unexplained_output_as_clean() {
        assert!(parse_jsonnetfmt(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_jsonnetfmt(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_jsonnetfmt(b"", b"boom\n", Some(1), &FILES).is_err());
        assert!(parse_jsonnetfmt(b"", b"x\n", Some(0), &FILES).is_err());
        assert!(parse_jsonnetfmt(b"/o/other.jsonnet\n", b"", Some(2), &FILES).is_err());
        assert!(parse_jsonnetfmt(&[0xff], b"", Some(0), &FILES).is_err());
        assert!(parse_jsonnetfmt(b"", b"", Some(3), &FILES).is_err());
    }

    #[test]
    fn jsonnetfmt_picks_the_longest_checked_file() {
        let findings = parse_jsonnetfmt(
            b"",
            b"/s/a.b:2:1 Unexpected\n",
            Some(1),
            &["/s/a", "/s/a.b"],
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "/s/a.b");
    }
}
