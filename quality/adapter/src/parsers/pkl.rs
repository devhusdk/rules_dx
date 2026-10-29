use super::{as_text, code_name, finding, known_native, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "pkl";
const TRAILER: &str = "An error occurred during formatting.";

/// Exit 0 is clean, 11 lists unformatted files, and 1 lists them and reports located errors.
pub fn parse_pkl(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let listed = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    let mut findings = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for line in listed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let file = known_native(TOOL, files, line)?;
        if seen.contains(&file) {
            continue;
        }
        seen.push(file);
        findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
    }
    match code {
        Some(0) if findings.is_empty() && errors.trim().is_empty() => Ok(findings),
        Some(11) if !findings.is_empty() && errors.trim().is_empty() => Ok(findings),
        Some(1) => {
            let before = findings.len();
            findings.extend(format_errors(errors, files)?);
            if findings.len() == before {
                return Err(ParseError::shape(TOOL, "exit 1 with no located errors"));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} with output", code_name(code)),
        )),
    }
}

fn format_errors(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    let mut closed = false;
    for line in stderr.lines().map(str::trim_end).filter(|l| !l.is_empty()) {
        if closed {
            return Err(ParseError::shape(
                TOOL,
                format!("output after the closing line: {line}"),
            ));
        }
        if line == TRAILER {
            closed = true;
            continue;
        }
        let Some((path, message)) = line
            .strip_prefix("Could not format `")
            .and_then(|rest| rest.split_once("`: "))
        else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognized error output: {line}"),
            ));
        };
        let file = known_native(TOOL, files, path)?;
        let (message, row, column) = split_location(message);
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.to_owned(),
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    if !closed {
        return Err(ParseError::shape(
            TOOL,
            "error output without the closing line",
        ));
    }
    Ok(findings)
}

/// The message and the start of its trailing ` at (<row>:<col> - <row>:<col>)` range.
fn split_location(message: &str) -> (&str, u64, u64) {
    let located: Option<(&str, u64, u64)> = message.strip_suffix(')').and_then(|rest| {
        let (text, range) = rest.rsplit_once(" at (")?;
        let (start, _) = range.split_once(" - ")?;
        let (row, column) = start.split_once(':')?;
        Some((text, row.parse().ok()?, column.parse().ok()?))
    });
    match located {
        Some((text, row, column)) => (text.trim_end_matches('.'), row.max(1), column.max(1)),
        None => (message, 1, 1),
    }
}

/// `pkl eval` of one module. Exit 0 with no output on stderr is clean. Exit 1 prints one
/// `Pkl Error` block; its first `at ... (file:///path, line N)` frame is the finding, which may
/// sit in an imported file. Every other exit or output shape is an error, never clean.
pub fn parse_pkl_eval(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    known: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    // Stdout holds the rendered module on success; it is size-checked and otherwise ignored.
    as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    match code {
        Some(0) if errors.trim().is_empty() => Ok(Vec::new()),
        Some(1) => eval_error(errors, known).map(|found| vec![found]),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected eval exit {} with output", code_name(code)),
        )),
    }
}

const EVAL_HEADER: &str = "\u{2013}\u{2013} Pkl Error \u{2013}\u{2013}";

fn eval_error(stderr: &str, known: &[&str]) -> Result<FileFinding, ParseError> {
    let lines: Vec<&str> = stderr.lines().map(str::trim_end).collect();
    if lines.first() != Some(&EVAL_HEADER) || lines[1..].contains(&EVAL_HEADER) {
        return Err(ParseError::shape(
            TOOL,
            "eval error output is not a single Pkl Error block",
        ));
    }
    let message = lines[1..]
        .iter()
        .take_while(|line| !line.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    if message.is_empty() {
        return Err(ParseError::shape(TOOL, "Pkl Error block without a message"));
    }
    let Some(at) = lines.iter().position(|line| line.starts_with("at ")) else {
        return Err(ParseError::shape(
            TOOL,
            format!("error without a located frame: {message}"),
        ));
    };
    let (path, row) = frame_location(lines[at])
        .ok_or_else(|| ParseError::shape(TOOL, format!("unlocated first frame: {}", lines[at])))?;
    let file = known_native(TOOL, known, &path)?;
    let column = frame_column(&lines[..at]);
    Ok(finding(
        TOOL,
        file,
        String::new(),
        message,
        ToolSeverity::Error,
        TextPosition {
            line: row.max(1),
            column,
        },
        ColumnEncoding::Scalars,
    ))
}

/// The file path and line of an `at <name> (file:///path, line N)` frame.
fn frame_location(frame: &str) -> Option<(String, u64)> {
    let inner = frame.strip_suffix(')')?.rsplit_once(" (")?.1;
    let (uri, row) = inner.rsplit_once(", line ")?;
    let path = percent_decode(uri.strip_prefix("file://")?)?;
    // A Windows drive path is spelled `file:///C:/dir/a.pkl`.
    let bytes = path.as_bytes();
    let path = if bytes.len() > 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b':'
    {
        path[1..].to_owned()
    } else {
        path
    };
    if !path.starts_with('/') && !path.as_bytes().get(1).is_some_and(|c| *c == b':') {
        return None;
    }
    Some((path, row.parse().ok()?))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The 1-based column of the first caret under the snippet line above the frame, or 1.
fn frame_column(before: &[&str]) -> u64 {
    let [.., snippet, carets] = before else {
        return 1;
    };
    let Some(gutter) = snippet.find("| ").map(|at| at + 2) else {
        return 1;
    };
    let Some(caret) = carets.find('^') else {
        return 1;
    };
    let offset = carets[..caret].chars().count().saturating_sub(gutter);
    offset as u64 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 4] = ["/s/main.pkl", "/s/lib/a.pkl", "/s/bad.pkl", "/s/open.pkl"];
    const BROKEN: &str = "Could not format `/s/bad.pkl`: Unexpected token `=`. at (1:5 - 1:6)\nCould not format `/s/open.pkl`: org.pkl.parser.ParserError: Missing `\"` delimiter.\nAn error occurred during formatting.\n";

    #[test]
    fn pkl_lists_unformatted_files() {
        let findings =
            parse_pkl(b"/s/main.pkl\n/s/lib/a.pkl\n", b"", Some(11), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/main.pkl");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert!(parse_pkl(b"", b"", Some(0), &FILES).expect("ok").is_empty());
    }

    #[test]
    fn pkl_locates_errors_next_to_unformatted_files() {
        let findings =
            parse_pkl(b"/s/lib/a.pkl\n", BROKEN.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[1].file, "/s/bad.pkl");
        assert_eq!(findings[1].finding.message, "Unexpected token `=`");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].finding.start.line, 1);
        assert_eq!(findings[1].finding.start.column, 5);
        assert_eq!(findings[2].file, "/s/open.pkl");
        assert_eq!(findings[2].finding.start.column, 1);
    }

    #[test]
    fn pkl_never_reports_a_failure_as_clean() {
        assert!(parse_pkl(b"", b"", Some(11), &FILES).is_err());
        assert!(parse_pkl(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_pkl(b"", b"", Some(2), &FILES).is_err());
        assert!(parse_pkl(b"", b"", None, &FILES).is_err());
        assert!(parse_pkl(b"/s/main.pkl\n", b"", Some(0), &FILES).is_err());
        assert!(parse_pkl(b"", b"warning\n", Some(0), &FILES).is_err());
        assert!(parse_pkl(&[0xff], b"", Some(0), &FILES).is_err());
        let crash = b"An unexpected error has occurred. Would you mind filing a bug report?\n\njava.lang.IndexOutOfBoundsException\n";
        assert!(parse_pkl(b"", crash, Some(1), &FILES).is_err());
        let io = b"IO error while reading `/s/gone.pkl`: /s/gone.pkl\nAn error occurred during formatting.\n";
        assert!(parse_pkl(b"", io, Some(1), &FILES).is_err());
        assert!(parse_pkl(b"", b"Could not format `/s/bad.pkl`: x\n", Some(1), &FILES).is_err());
    }

    #[test]
    fn pkl_cannot_report_a_file_outside_the_run() {
        let err = parse_pkl(b"/elsewhere.pkl\n", b"", Some(11), &FILES).expect_err("fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let stderr = b"Could not format `/elsewhere.pkl`: x at (1:1 - 1:2)\nAn error occurred during formatting.\n";
        let err = parse_pkl(b"", stderr, Some(1), &FILES).expect_err("fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }

    #[test]
    fn pkl_backslash_paths_resolve_only_on_windows() {
        let files = ["m/a.pkl"];
        let listed = parse_pkl(b"m\\a.pkl\nm/a.pkl\n", b"", Some(11), &files);
        let stderr = b"Could not format `m\\a.pkl`: x at (2:3 - 2:4)\nAn error occurred during formatting.\n";
        let located = parse_pkl(b"", stderr, Some(1), &files);
        if cfg!(windows) {
            assert_eq!(listed.expect("ok").len(), 1);
            assert_eq!(located.expect("ok")[0].file, "m/a.pkl");
        } else {
            assert!(matches!(listed, Err(ParseError::UnknownFile { .. })));
            assert!(matches!(located, Err(ParseError::UnknownFile { .. })));
        }
    }

    const OWNED: &str = "/s/main.pkl";
    const KNOWN: [&str; 2] = ["/s/main.pkl", "/s/lib/dep.pkl"];
    const FRAMES: &str = "\n131 | renderer.renderDocument(value)\n      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#Module.output.text (https://github.com/apple/pkl/blob/0.32.1/stdlib/base.pkl#L131)\n";

    fn block(message: &str, snippet: &str, carets: &str, file: &str) -> String {
        format!(
            "\u{2013}\u{2013} Pkl Error \u{2013}\u{2013}\n{message}\n\n{snippet}\n{carets}\nat main#x (file://{file}, line 1)\n{FRAMES}"
        )
    }

    #[test]
    fn eval_clean_is_exit_zero_with_quiet_stderr() {
        assert!(parse_pkl_eval(b"x = 1\n", b"", Some(0), &KNOWN)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn eval_reports_a_type_mismatch_with_its_location() {
        let err = block(
            "Expected value of type `Int`, but got type `String`.\nValue: \"a\"",
            "1 | x: Int = \"a\"",
            "       ^^^",
            OWNED,
        );
        let found = parse_pkl_eval(b"", err.as_bytes(), Some(1), &KNOWN).expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, OWNED);
        assert_eq!(found[0].finding.start.line, 1);
        assert_eq!(found[0].finding.start.column, 4);
        assert!(found[0]
            .finding
            .message
            .starts_with("Expected value of type"));
    }

    #[test]
    fn eval_reports_an_undefined_property() {
        let err = block("Cannot find property `y`.", "1 | x = y", "        ^", OWNED);
        let found = parse_pkl_eval(b"", err.as_bytes(), Some(1), &KNOWN).expect("ok");
        assert_eq!(found[0].finding.message, "Cannot find property `y`.");
        assert_eq!(found[0].finding.start.column, 9 - 4);
    }

    #[test]
    fn eval_returns_an_error_located_in_an_imported_file_with_its_path() {
        let err = block(
            "Cannot find property `y`.",
            "1 | v = y",
            "        ^",
            "/s/lib/dep.pkl",
        );
        let found = parse_pkl_eval(b"", err.as_bytes(), Some(1), &KNOWN).expect("ok");
        assert_eq!(found[0].file, "/s/lib/dep.pkl");
    }

    #[test]
    fn eval_decodes_percent_escapes_in_paths() {
        let known = ["/s/a b.pkl"];
        let err = block("Oops.", "1 | x", "    ^", "/s/a%20b.pkl");
        let found = parse_pkl_eval(b"", err.as_bytes(), Some(1), &known).expect("ok");
        assert_eq!(found[0].file, "/s/a b.pkl");
    }

    #[test]
    fn eval_never_reports_an_unexpected_shape_as_clean() {
        let header = "\u{2013}\u{2013} Pkl Error \u{2013}\u{2013}\n";
        let no_frame = format!("{header}Refusing to load module `https://x/y.pkl`.\n");
        assert!(parse_pkl_eval(b"", no_frame.as_bytes(), Some(1), &KNOWN).is_err());
        assert!(parse_pkl_eval(b"", b"", Some(1), &KNOWN).is_err());
        assert!(parse_pkl_eval(b"", b"warning\n", Some(0), &KNOWN).is_err());
        assert!(parse_pkl_eval(b"", b"", Some(2), &KNOWN).is_err());
        assert!(parse_pkl_eval(b"", b"", None, &KNOWN).is_err());
        assert!(parse_pkl_eval(&[0xff], b"", Some(0), &KNOWN).is_err());
        let ok = block("Oops.", "1 | x", "    ^", OWNED);
        let twice = format!("{ok}{ok}");
        assert!(parse_pkl_eval(b"", twice.as_bytes(), Some(1), &KNOWN).is_err());
        let junk = format!("crash\n{ok}");
        assert!(parse_pkl_eval(b"", junk.as_bytes(), Some(1), &KNOWN).is_err());
        let stdlib = format!("{header}Oops.\n{FRAMES}");
        assert!(parse_pkl_eval(b"", stdlib.as_bytes(), Some(1), &KNOWN).is_err());
    }

    #[test]
    fn eval_cannot_report_a_file_outside_the_run() {
        let err = block("Oops.", "1 | x", "    ^", "/elsewhere.pkl");
        let got = parse_pkl_eval(b"", err.as_bytes(), Some(1), &KNOWN).expect_err("fails");
        assert!(matches!(got, ParseError::UnknownFile { .. }));
    }
}
