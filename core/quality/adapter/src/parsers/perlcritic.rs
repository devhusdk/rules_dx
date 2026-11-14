use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{FileFinding, ParseError, as_text, code_name, finding, known};

const TOOL: &str = "perlcritic";

/// Findings from one batched `perlcritic` run in the fixed tab-separated format.
///
/// Every stdout line must carry the format's sentinel and name one of `files`. Perl::Critic exits
/// 2 exactly when it printed a violation, so exit 0 with lines, exit 2 without lines and any
/// other exit (1: no files, 3: a file could not be critiqued) are errors, never a clean run.
/// Lines are the physical file and line, so `#line` directives cannot redirect a finding. The
/// column is Perl::Critic's visual column: 1-based with tabs expanded to multiples of 8.
pub fn parse_perlcritic(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let mut out = Vec::new();
    for line in text.lines() {
        out.push(violation(line, files)?);
    }
    match (code, out.is_empty()) {
        (Some(0), true) | (Some(2), false) => Ok(out),
        (Some(0), false) => Err(shape(format!(
            "exit 0 with {} violation lines (want none)",
            out.len()
        ))),
        (Some(2), true) => Err(shape("exit 2 without a violation line".to_owned())),
        _ => Err(shape(format!("exit {} (want 0 or 2)", code_name(code)))),
    }
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

fn violation(line: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let bad = |detail: &str| shape(format!("{detail}: {line}"));
    let fields: Vec<&str> = line.splitn(PERLCRITIC_FIELDS, '\t').collect();
    let [sentinel, path, line_text, column_text, severity_text, policy, message] =
        fields.as_slice()
    else {
        return Err(bad("line outside the pinned format"));
    };
    if *sentinel != PERLCRITIC_SENTINEL {
        return Err(bad("line outside the pinned format"));
    }
    let file = known(TOOL, files, path)?;
    let number = |text: &str| -> Result<u64, ParseError> {
        match text.parse::<u64>() {
            Ok(value) if value > 0 => Ok(value),
            _ => Err(bad("position is not a 1-based number")),
        }
    };
    let position = TextPosition {
        line: number(line_text)?,
        column: number(column_text)?,
    };
    // Severities 1 to 5 all parse.
    let severity = match *severity_text {
        "5" | "4" => ToolSeverity::Error,
        "3" | "2" | "1" => ToolSeverity::Warning,
        _ => return Err(bad("unknown severity")),
    };
    if policy.is_empty() {
        return Err(bad("violation without a policy name"));
    }
    Ok(finding(
        TOOL,
        file,
        (*policy).to_owned(),
        message.trim().to_owned(),
        severity,
        position,
        ColumnEncoding::ScalarsTabs { width: 8 },
    ))
}

const PERLCRITIC_SENTINEL: &str = "dx";
const PERLCRITIC_FIELDS: usize = 7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::PERLCRITIC_FORMAT;

    const A: &str = "/s/a.pl";
    const B: &str = "/s/b.pl";
    const HIT: &[u8] = b"dx\t/s/a.pl\t4\t9\t5\tTestingAndDebugging::RequireUseStrict\tCode before strictures are enabled\n";

    fn run(stdout: &[u8], code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_perlcritic(stdout, Some(code), &[A, B])
    }

    #[test]
    fn the_format_matches_the_parser_grammar() {
        assert_eq!(PERLCRITIC_FORMAT.split(r"\t").count(), PERLCRITIC_FIELDS);
        assert_eq!(
            PERLCRITIC_FORMAT.split(r"\t").next(),
            Some(PERLCRITIC_SENTINEL)
        );
        assert!(PERLCRITIC_FORMAT.ends_with(r"\n"));
        assert!(!PERLCRITIC_FORMAT.contains(['\t', '\n', '\r']));
    }

    #[test]
    fn perlcritic_is_clean_on_exit_zero_without_lines() {
        assert!(run(b"", 0).expect("parsed").is_empty());
    }

    #[test]
    fn perlcritic_reports_a_violation_with_its_policy_and_visual_column() {
        let found = run(HIT, 2).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, A);
        assert_eq!(
            found[0].finding.rule_id,
            "TestingAndDebugging::RequireUseStrict"
        );
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(
            found[0].finding.encoding,
            ColumnEncoding::ScalarsTabs { width: 8 }
        );
        assert_eq!(
            found[0].finding.message,
            "Code before strictures are enabled"
        );
    }

    #[test]
    fn perlcritic_keeps_tabs_and_colons_inside_the_message() {
        let line = b"dx\t/s/b.pl\t1\t1\t5\tP::Q\ta:\tb\n";
        assert_eq!(run(line, 2).expect("parsed")[0].finding.message, "a:\tb");
    }

    #[test]
    fn perlcritic_never_trusts_an_exit_that_disagrees_with_its_output() {
        assert!(run(HIT, 0).is_err());
        assert!(run(b"", 2).is_err());
        assert!(run(b"", 1).is_err());
        assert!(run(b"", 3).is_err());
        assert!(run(HIT, 3).is_err());
        assert!(parse_perlcritic(b"", None, &[A]).is_err());
    }

    #[test]
    fn perlcritic_fails_closed_on_anything_outside_the_format() {
        assert!(run(b"boom\n", 2).is_err());
        assert!(run(b"dx\t/s/a.pl\t1\n", 2).is_err());
        assert!(run(b"dx\t/s/c.pl\t1\t1\t5\tP\tm\n", 2).is_err());
        assert!(run(b"dx\t/s/a.pl\t0\t1\t5\tP\tm\n", 2).is_err());
        assert!(run(b"dx\t/s/a.pl\tx\t1\t5\tP\tm\n", 2).is_err());
        assert!(run(b"dx\t/s/a.pl\t1\t1\t9\tP\tm\n", 2).is_err());
        assert!(run(b"dx\t/s/a.pl\t1\t1\t5\t\tm\n", 2).is_err());
        assert!(run(b"WARN\n\xff\n", 2).is_err());
    }
}
