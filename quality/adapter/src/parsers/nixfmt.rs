use super::{as_text, code_name, finding, known_native, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "nixfmt";

/// `nixfmt --check` exits 0 with no output when every file is formatted. It exits 1 for unformatted
/// files, syntax errors, I/O errors and usage errors alike, so exit 1 is read by classifying
/// stderr into records: `<path>: not formatted`, or `<path>:<line>:<col>:` followed by its
/// megaparsec continuation lines. Anything else (stray text, an unowned path, a colour-coded
/// warning, an uncaught exception) is a shape error and never reads as clean. Exit 2 is SIGINT
/// and any other exit, or no exit, is an error.
pub fn parse_nixfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    if !out.is_empty() {
        return Err(ParseError::shape(TOOL, "unexpected stdout"));
    }
    match code {
        Some(0) if err.is_empty() => Ok(Vec::new()),
        Some(0) => Err(ParseError::shape(TOOL, "exit 0 with output")),
        Some(1) => records(err, files),
        _ => Err(ParseError::shape(TOOL, format!("exit {}", code_name(code)))),
    }
}

enum Header<'a> {
    Unformatted(&'a str),
    Located(&'a str, u64, u64),
}

fn records(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    if stderr.contains('\u{1b}') {
        return Err(ParseError::shape(TOOL, "colour-coded warning on stderr"));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut open: Option<(&str, u64, u64, Vec<&str>)> = None;
    for line in stderr.lines() {
        match header(line, files)? {
            Some(next) => {
                close(&mut open, &mut findings);
                match next {
                    Header::Unformatted(file) => {
                        if !findings.iter().any(|f| f.file == file) {
                            findings.push(unformatted_at(
                                TOOL,
                                file,
                                1,
                                1,
                                ColumnEncoding::Scalars,
                            ));
                        }
                    }
                    Header::Located(file, row, column) => {
                        open = Some((file, row, column, Vec::new()));
                    }
                }
            }
            None => match open.as_mut() {
                Some((_, _, _, body)) if !line.starts_with("nixfmt:") => body.push(line),
                _ => {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("unrecognized stderr line: {line}"),
                    ))
                }
            },
        }
    }
    close(&mut open, &mut findings);
    if findings.is_empty() {
        return Err(ParseError::shape(TOOL, "exit 1 without a record"));
    }
    Ok(findings)
}

fn close(open: &mut Option<(&str, u64, u64, Vec<&str>)>, findings: &mut Vec<FileFinding>) {
    let Some((file, row, column, body)) = open.take() else {
        return;
    };
    let message = body
        .iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with(' ') && !is_source_line(line))
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("; ");
    findings.push(finding(
        TOOL,
        file,
        String::new(),
        if message.is_empty() {
            "syntax error".to_owned()
        } else {
            message
        },
        ToolSeverity::Error,
        TextPosition { line: row, column },
        ColumnEncoding::Scalars,
    ));
}

/// A quoted source line of the error excerpt: `<line> | <text>`.
fn is_source_line(line: &str) -> bool {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && line[digits..].starts_with(" |")
}

/// A record header, `None` for a continuation line, or an error for a header naming an unowned path.
fn header<'a>(line: &str, files: &[&'a str]) -> Result<Option<Header<'a>>, ParseError> {
    if let Some(path) = line.strip_suffix(": not formatted") {
        return Ok(Some(Header::Unformatted(known_native(TOOL, files, path)?)));
    }
    let Some(rest) = line.strip_suffix(':') else {
        return Ok(None);
    };
    let Some((rest, column)) = rest.rsplit_once(':') else {
        return Ok(None);
    };
    let Some((path, row)) = rest.rsplit_once(':') else {
        return Ok(None);
    };
    let (Ok(row), Ok(column)) = (row.parse::<u64>(), column.parse::<u64>()) else {
        return Ok(None);
    };
    if path.is_empty() || path.starts_with(' ') {
        return Ok(None);
    }
    Ok(Some(Header::Located(
        known_native(TOOL, files, path)?,
        row.max(1),
        column.max(1),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/s/a.nix", "/s/b.nix", "/s/c.nix"];
    const BROKEN: &str = "/s/b.nix:1:7:\n  |\n1 | { a = ; }\n  |       ^^^^\nunexpected \"; }<newline>\"\nexpecting expression\n\n";

    fn run(err: &str, code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_nixfmt(b"", err.as_bytes(), code, &FILES)
    }

    #[test]
    fn nixfmt_clean_is_empty() {
        assert!(run("", Some(0)).expect("ok").is_empty());
    }

    #[test]
    fn nixfmt_reports_unformatted_files() {
        let findings = run(
            "/s/a.nix: not formatted\n/s/c.nix: not formatted\n",
            Some(1),
        )
        .expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.nix");
        assert_eq!(findings[1].file, "/s/c.nix");
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn nixfmt_locates_a_syntax_error() {
        let findings = run(BROKEN, Some(1)).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/b.nix");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 7);
        assert!(findings[0].finding.message.contains("expecting expression"));
    }

    #[test]
    fn nixfmt_reads_mixed_records() {
        let err = format!("/s/a.nix: not formatted\n{BROKEN}");
        let findings = run(&err, Some(1)).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn nixfmt_never_reports_a_failure_as_clean() {
        assert!(run("", Some(1)).is_err());
        assert!(run("", Some(2)).is_err());
        assert!(run("", None).is_err());
        assert!(run("", Some(3)).is_err());
        assert!(run("boom\n", Some(1)).is_err());
        assert!(run("warning\n", Some(0)).is_err());
        assert!(run("/s/a.nix: not formatted\n", Some(0)).is_err());
        assert!(run("/s/other.nix: not formatted\n", Some(1)).is_err());
        assert!(run("/s/other.nix:1:1:\n  |\n", Some(1)).is_err());
        assert!(run("stray\n/s/a.nix: not formatted\n", Some(1)).is_err());
        assert!(run(
            "\u{1b}[33m/s/a.nix is a symbolic link\u{1b}[39m\n/s/a.nix: not formatted\n",
            Some(1)
        )
        .is_err());
        assert!(run(
            &format!("{BROKEN}nixfmt: Uncaught exception ghc-internal:IOException:\n"),
            Some(1)
        )
        .is_err());
        assert!(parse_nixfmt(b"x\n", b"", Some(0), &FILES).is_err());
        assert!(parse_nixfmt(b"x\n", b"/s/a.nix: not formatted\n", Some(1), &FILES).is_err());
        assert!(parse_nixfmt(b"", &[0xff], Some(1), &FILES).is_err());
        assert!(parse_nixfmt(&[0xff], b"", Some(0), &FILES).is_err());
    }
}
