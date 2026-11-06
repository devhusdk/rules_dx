use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{FileFinding, ParseError, as_text, code_name, finding, located, missing};

const TOOL: &str = "chktex";
const UNREADABLE: &str = "Unable to open the TeX file";
const FIELDS: usize = 6;

/// Findings from one `chktex` run on a single file in the fixed colon-separated format.
///
/// chktex raises its exit to the worst level it printed: 2 for a warning, 3 for an error, and
/// `Message` lines never raise it. The exit must therefore equal the worst printed level, so a
/// crash or a mismatch is never read as a clean run. Any stderr text is an error, and an
/// unreadable file is an incomplete run. The column is chktex's 1-based byte offset.
pub fn parse_chktex(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if files.len() != 1 {
        return Err(ParseError::shape(
            TOOL,
            format!("expected one checked file, got {}", files.len()),
        ));
    }
    let diagnostics = as_text(TOOL, stderr)?;
    if let Some(line) = diagnostics.lines().map(str::trim).find(|l| !l.is_empty()) {
        if line.contains(UNREADABLE) {
            return Err(ParseError::Incomplete {
                tool: TOOL,
                detail: line.to_owned(),
            });
        }
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected stderr: {line}"),
        ));
    }
    let mut out = Vec::new();
    let mut worst = 0;
    for line in as_text(TOOL, stdout)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let (found, level) = diagnostic(line, files)?;
        worst = worst.max(level);
        out.push(found);
    }
    if code == Some(worst) {
        Ok(out)
    } else {
        Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} (want {worst} for the printed diagnostics)",
                code_name(code)
            ),
        ))
    }
}

fn diagnostic(line: &str, files: &[&str]) -> Result<(FileFinding, i32), ParseError> {
    let (file, rest) = located(TOOL, line, files)?;
    let fields: Vec<&str> = rest.splitn(FIELDS, ':').collect();
    if fields.len() != FIELDS {
        return Err(missing(TOOL, "diagnostic", line));
    }
    let number = |text: &str, least: u64| match text.parse::<u64>() {
        Ok(value) if value >= least => Ok(value),
        _ => Err(missing(TOOL, "position", line)),
    };
    let row = number(fields[0], 1)?;
    let column = number(fields[1], 1)?;
    let length = number(fields[2], 0)?;
    let (severity, level) = match fields[3] {
        "Error" => (ToolSeverity::Error, 3),
        "Warning" => (ToolSeverity::Warning, 2),
        "Message" => (ToolSeverity::Warning, 0),
        _ => return Err(missing(TOOL, "kind", line)),
    };
    let message = fields[5].trim();
    if fields[4].is_empty() || message.is_empty() {
        return Err(missing(TOOL, "diagnostic", line));
    }
    let mut found = finding(
        TOOL,
        file,
        fields[4].to_owned(),
        message.to_owned(),
        severity,
        TextPosition { line: row, column },
        ColumnEncoding::Utf8,
    );
    if length > 0 {
        found.finding.end = Some(TextPosition {
            line: row,
            column: column + length,
        });
    }
    Ok((found, level))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CHKTEX_FORMAT;

    const FILES: [&str; 1] = ["/s/doc/a.tex"];
    const WARN: &str = "/s/doc/a.tex:3:5:2:Warning:24:Delete this space: it is odd.\n";
    const ERR: &str = "/s/doc/a.tex:1:1:0:Error:11:Bad thing\n";
    const NOTE: &str = "/s/doc/a.tex:2:1:0:Message:1:Note\n";

    fn run(stdout: &str, stderr: &str, code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_chktex(stdout.as_bytes(), stderr.as_bytes(), code, &FILES)
    }

    #[test]
    fn the_format_matches_the_parser_grammar() {
        assert_eq!(CHKTEX_FORMAT.split(':').count(), FIELDS + 1);
        assert!(CHKTEX_FORMAT.ends_with('\n'));
        assert!(!CHKTEX_FORMAT.contains("\\n"));
    }

    #[test]
    fn chktex_clean_run() {
        assert!(run("", "", Some(0)).expect("parsed").is_empty());
    }

    #[test]
    fn chktex_warning_keeps_rule_position_end_and_colons() {
        let found = run(WARN, "", Some(2)).expect("parsed");
        assert_eq!(found.len(), 1);
        let item = &found[0].finding;
        assert_eq!(found[0].file, "/s/doc/a.tex");
        assert_eq!(item.rule_id, "24");
        assert_eq!(item.severity, ToolSeverity::Warning);
        assert_eq!((item.start.line, item.start.column), (3, 5));
        assert_eq!(item.end, Some(TextPosition { line: 3, column: 7 }));
        assert_eq!(item.encoding, ColumnEncoding::Utf8);
        assert_eq!(item.message, "Delete this space: it is odd.");
    }

    #[test]
    fn chktex_error_at_exit_three() {
        let found = run(&format!("{WARN}{ERR}"), "", Some(3)).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].finding.severity, ToolSeverity::Error);
        assert_eq!(found[1].finding.end, None);
    }

    #[test]
    fn chktex_message_only_output_is_exit_zero() {
        assert_eq!(run(NOTE, "", Some(0)).expect("parsed").len(), 1);
        assert!(run(NOTE, "", Some(2)).is_err());
    }

    #[test]
    fn chktex_unreadable_file_is_incomplete() {
        let error = run(
            "",
            "ChkTeX: Unable to open the TeX file `a.tex'.\n",
            Some(0),
        )
        .expect_err("unreadable");
        assert!(matches!(error, ParseError::Incomplete { .. }), "{error}");
        assert!(matches!(
            run("", "other\n", Some(0)),
            Err(ParseError::Shape { .. })
        ));
    }

    #[test]
    fn chktex_unknown_exits_are_never_clean() {
        for code in [Some(1), Some(4), None] {
            assert!(run("", "", code).is_err());
            assert!(run(WARN, "", code).is_err());
        }
    }

    #[test]
    fn chktex_exit_must_match_the_printed_level() {
        assert!(run("", "", Some(2)).is_err());
        assert!(run(ERR, "", Some(2)).is_err());
        assert!(run(WARN, "", Some(0)).is_err());
    }

    #[test]
    fn chktex_rejects_malformed_lines() {
        assert!(run("junk\n", "", Some(2)).is_err());
        assert!(run("/other.tex:1:1:0:Warning:1:m\n", "", Some(2)).is_err());
        assert!(run("/s/doc/a.tex:1:1:0:Fatal:1:m\n", "", Some(2)).is_err());
        assert!(run("/s/doc/a.tex:0:1:0:Warning:1:m\n", "", Some(2)).is_err());
        assert!(run("/s/doc/a.tex:1:1:0:Warning::m\n", "", Some(2)).is_err());
    }

    #[test]
    fn chktex_checks_exactly_one_file() {
        assert!(parse_chktex(b"", b"", Some(0), &[]).is_err());
        assert!(parse_chktex(b"", b"", Some(0), &["a.tex", "b.tex"]).is_err());
    }
}
