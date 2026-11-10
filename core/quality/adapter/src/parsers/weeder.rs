use super::{FileFinding, ParseError, as_text, code_name, known_native};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "weeder";

/// The exit weeder uses when it found unused declarations.
const WEEDS_EXIT: i32 = 228;

/// Weeder says this for each root pattern that matches nothing, as `Main.main` does in a library.
const UNMATCHED_ROOT: &str = "no declaration matches roots entry";

fn weed(line: &str) -> Option<(&str, u64, u64, &str)> {
    let (_unit, rest) = line.split_once(": ")?;
    let (location, declaration) = rest.split_once(": ")?;
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line_number = parts.next()?.parse().ok()?;
    Some((parts.next()?, line_number, column, declaration.trim()))
}

/// Parses `weeder` output: one `<unit>: path:line:col: <declaration>` line per weed.
///
/// Weeds are reported as info. A line that is not a weed, or a weed in a file that was not
/// passed, is an error and never a clean run.
pub fn parse_weeder(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    if !matches!(code, Some(0 | WEEDS_EXIT)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stderr).trim()
            ),
        ));
    }
    let mut findings = Vec::new();
    for line in text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with(UNMATCHED_ROOT))
    {
        let (path, line_number, column, message) = weed(line)
            .filter(|(_, line_number, column, message)| {
                *line_number > 0 && *column > 0 && !message.is_empty()
            })
            .ok_or_else(|| ParseError::shape(TOOL, format!("unexpected line: {line}")))?;
        findings.push(FileFinding {
            file: known_native(TOOL, files, path)?.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: "unused".to_owned(),
                message: format!("{message} is unused"),
                severity: ToolSeverity::Info,
                start: TextPosition {
                    line: line_number,
                    column,
                },
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} weeds", code_name(code), findings.len()),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/src/Lib.hs", "/s/src/Main.hs"];

    fn parse(stdout: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_weeder(stdout.as_bytes(), b"", Some(code), &FILES)
    }

    #[test]
    fn weeder_is_clean_on_empty_output_and_exit_zero() {
        assert!(parse("", 0).expect("parsed").is_empty());
    }

    #[test]
    fn weeder_reports_an_unused_binding_as_info() {
        let findings =
            parse("lib-0.1-inplace: /s/src/Lib.hs:7:1: orphan\n", WEEDS_EXIT).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/src/Lib.hs");
        assert_eq!(findings[0].finding.tool_id, "weeder");
        assert_eq!(findings[0].finding.rule_id, "unused");
        assert_eq!(findings[0].finding.message, "orphan is unused");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 7, column: 1 }
        );
    }

    #[test]
    fn weeder_ignores_unmatched_root_notices() {
        let out = "no declaration matches roots entry \"^Main\\\\.main$\"\n";
        assert!(parse(out, 0).expect("parsed").is_empty());
    }

    #[test]
    fn weeder_fails_closed() {
        let weed = "lib-0.1-inplace: /s/src/Lib.hs:7:1: orphan\n";
        assert!(parse(weed, 0).is_err());
        assert!(parse("", WEEDS_EXIT).is_err());
        assert!(parse("", 1).is_err());
        assert!(parse_weeder(b"", b"", None, &FILES).is_err());
        assert!(parse("not a weed\n", WEEDS_EXIT).is_err());
        assert!(parse("u: /s/src/Lib.hs:0:1: x\n", WEEDS_EXIT).is_err());
        assert!(parse("u: /s/src/Other.hs:7:1: x\n", WEEDS_EXIT).is_err());
        assert!(parse_weeder(&[0xff, 0xfe], b"", Some(WEEDS_EXIT), &FILES).is_err());
    }
}
