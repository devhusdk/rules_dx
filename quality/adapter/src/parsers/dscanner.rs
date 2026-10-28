use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, finding, known, FileFinding, ParseError};

const TOOL: &str = "dscanner";

/// Findings from one batched `dscanner --styleCheck` run in the fixed tab-separated format.
///
/// Every stdout line must carry the format's sentinel and name one of `files`. D-Scanner exits 1
/// exactly when it printed a diagnostic, so exit 0 with lines, exit 1 without lines and any other
/// exit are errors, never a clean run. Syntax errors arrive as `error` lines with an empty check
/// name and become the rule `syntax`. Positions are 1-based UTF-8 byte offsets.
pub fn parse_dscanner(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let mut out = Vec::new();
    for line in text.lines() {
        out.push(diagnostic(line, files)?);
    }
    match (code, out.is_empty()) {
        (Some(0), true) | (Some(1), false) => Ok(out),
        (Some(0), false) => Err(shape(format!(
            "exit 0 with {} diagnostic lines (want none)",
            out.len()
        ))),
        (Some(1), true) => Err(shape("exit 1 without a diagnostic line".to_owned())),
        _ => Err(shape(format!("exit {} (want 0 or 1)", code_name(code)))),
    }
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

fn diagnostic(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let bad = |detail: &str| shape(format!("{detail}: {line}"));
    let fields: Vec<&str> = line.splitn(DSCANNER_FIELDS, '\t').collect();
    if fields.len() != DSCANNER_FIELDS || fields[0] != DSCANNER_SENTINEL {
        return Err(bad("line outside the pinned format"));
    }
    let file = known(TOOL, files, fields[1])?;
    let number = |text: &str| -> Result<u64, ParseError> {
        match text.parse::<u64>() {
            Ok(value) if value > 0 => Ok(value),
            _ => Err(bad("position is not a 1-based number")),
        }
    };
    let position = TextPosition {
        line: number(fields[2])?,
        column: number(fields[3])?,
    };
    number(fields[4])?;
    number(fields[5])?;
    let severity = match fields[6] {
        "error" => ToolSeverity::Error,
        "warn" => ToolSeverity::Warning,
        _ => return Err(bad("unknown severity")),
    };
    let rule = match (fields[7], severity) {
        ("", ToolSeverity::Error) => "syntax",
        ("", _) => return Err(bad("diagnostic without a check name")),
        (name, _) => name,
    };
    Ok(finding(
        TOOL,
        file,
        rule.to_owned(),
        fields[8].trim().to_owned(),
        severity,
        position,
        ColumnEncoding::Utf8,
    ))
}

const DSCANNER_SENTINEL: &str = "dx";
const DSCANNER_FIELDS: usize = 9;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::DSCANNER_FORMAT;

    const A: &str = "/s/a.d";
    const B: &str = "/s/b.d";
    const WARN: &[u8] =
        b"dx\t/s/a.d\t4\t9\t4\t10\twarn\tunused_variable_check\tVariable x is never used.\n";

    fn run(stdout: &[u8], code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_dscanner(stdout, Some(code), &[A, B])
    }

    #[test]
    fn the_format_matches_the_parser_grammar() {
        assert_eq!(DSCANNER_FORMAT.split('\t').count(), DSCANNER_FIELDS);
        assert_eq!(DSCANNER_FORMAT.split('\t').next(), Some(DSCANNER_SENTINEL));
    }

    #[test]
    fn dscanner_is_clean_on_exit_zero_without_lines() {
        assert!(run(b"", 0).expect("parsed").is_empty());
    }

    #[test]
    fn dscanner_reports_a_warning_with_its_check_name_and_byte_column() {
        let found = run(WARN, 1).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, A);
        assert_eq!(found[0].finding.rule_id, "unused_variable_check");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Utf8);
        assert_eq!(found[0].finding.message, "Variable x is never used.");
    }

    #[test]
    fn dscanner_maps_a_nameless_error_to_a_syntax_error() {
        let line = b"dx\t/s/a.d\t1\t9\t1\t9\terror\t\tinvalid declaration\n";
        let found = run(line, 1).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "syntax");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn dscanner_keeps_tabs_inside_the_message() {
        let line = b"dx\t/s/a.d\t1\t1\t1\t2\twarn\tn\ta\tb\n";
        assert_eq!(run(line, 1).expect("parsed")[0].finding.message, "a\tb");
    }

    #[test]
    fn dscanner_never_trusts_an_exit_that_disagrees_with_its_output() {
        assert!(run(WARN, 0).is_err());
        assert!(run(b"", 1).is_err());
        assert!(run(b"", 2).is_err());
        assert!(parse_dscanner(b"", None, &[A]).is_err());
        assert!(run(WARN, 2).is_err());
    }

    #[test]
    fn dscanner_fails_closed_on_anything_outside_the_format() {
        assert!(run(b"boom\n", 1).is_err());
        assert!(run(b"dx\t/s/a.d\t1\n", 1).is_err());
        assert!(run(b"dx\t/s/c.d\t1\t1\t1\t2\twarn\tn\tm\n", 1).is_err());
        assert!(run(b"dx\t/s/a.d\t0\t1\t1\t2\twarn\tn\tm\n", 1).is_err());
        assert!(run(b"dx\t/s/a.d\tx\t1\t1\t2\twarn\tn\tm\n", 1).is_err());
        assert!(run(b"dx\t/s/a.d\t1\t1\t1\t2\tinfo\tn\tm\n", 1).is_err());
        assert!(run(b"dx\t/s/a.d\t1\t1\t1\t2\twarn\t\tm\n", 1).is_err());
        assert!(run(b"WARN\n\xff\n", 1).is_err());
    }
}
