use super::{as_text, finding, missing, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

struct Diagnostic<'a> {
    path: &'a str,
    line: u64,
    column: u64,
    rule: &'a str,
    message: String,
}

fn malformed(tool: &'static str, line: &str) -> ParseError {
    ParseError::shape(tool, format!("malformed diagnostic: {line:?}"))
}

/// One `path(line,col): error TSnnnn: message` line of `tsc --pretty false`.
fn diagnostic<'a>(tool: &'static str, line: &'a str) -> Result<Diagnostic<'a>, ParseError> {
    let (head, tail) = line
        .split_once("): ")
        .ok_or_else(|| missing(tool, "diagnostic location", line))?;
    let open = head
        .rfind('(')
        .ok_or_else(|| missing(tool, "diagnostic location", line))?;
    let (path, position) = head.split_at(open);
    let (line_text, column_text) = position[1..]
        .split_once(',')
        .ok_or_else(|| missing(tool, "diagnostic location", line))?;
    let (severity, rest) = tail
        .split_once(' ')
        .ok_or_else(|| missing(tool, "diagnostic", line))?;
    if severity != "error" {
        return Err(ParseError::shape(
            tool,
            format!("unknown severity in {line:?}"),
        ));
    }
    let (rule, message) = rest
        .split_once(": ")
        .ok_or_else(|| missing(tool, "diagnostic", line))?;
    let code = rule.strip_prefix("TS").unwrap_or_default();
    let (Ok(line_no), Ok(column)) = (line_text.parse::<u64>(), column_text.parse::<u64>()) else {
        return Err(malformed(tool, line));
    };
    if line_no < 1
        || column < 1
        || path.is_empty()
        || code.is_empty()
        || !code.bytes().all(|byte| byte.is_ascii_digit())
        || message.is_empty()
    {
        return Err(malformed(tool, line));
    }
    Ok(Diagnostic {
        path,
        line: line_no,
        column,
        rule,
        message: message.to_owned(),
    })
}

/// Whether a line is the trailing `Found N errors` summary some tsc modes print.
fn is_summary(line: &str) -> bool {
    line.strip_prefix("Found ")
        .is_some_and(|rest| rest.contains(" error"))
}

/// Parses `tsc --pretty false` output for the files in `files`.
///
/// Diagnostics in other files (dependencies, generated sources) are dropped. Location-less
/// diagnostics (`error TS5083: ...`) and a failing exit with no diagnostic in `files` are errors,
/// so a project that does not typecheck is never reported clean.
pub fn parse_tsc_format(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut diagnostics: Vec<Diagnostic<'_>> = Vec::new();
    for line in as_text(tool, stdout)?.lines() {
        if line.trim().is_empty() || is_summary(line.trim()) {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            let last = diagnostics
                .last_mut()
                .ok_or_else(|| missing(tool, "diagnostic", line))?;
            last.message.push('\n');
            last.message.push_str(line.trim());
            continue;
        }
        diagnostics.push(diagnostic(tool, line)?);
    }
    let findings: Vec<FileFinding> = diagnostics
        .into_iter()
        .filter_map(|found| {
            let file = files.iter().find(|file| **file == found.path)?;
            Some(finding(
                tool,
                file,
                found.rule.to_owned(),
                found.message,
                ToolSeverity::Error,
                TextPosition {
                    line: found.line,
                    column: found.column,
                },
                ColumnEncoding::Utf16,
            ))
        })
        .collect();
    require_findings(tool, &findings, code)?;
    Ok(findings)
}

pub fn parse_tsc(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    parse_tsc_format("tsc", stdout, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from `tsc 5.9.3 --project tsconfig.json --noEmit --pretty false`.
    const REAL: &str = "sub/a.ts(1,14): error TS2322: Type 'string' is not assignable to type 'number'.\nsub/a.ts(3,3): error TS2352: Conversion of type '{ a: number; b: { c: string; }; }' to type '{ a: number; b: string; }' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first.\n  Types of property 'b' are incompatible.\n    Type '{ c: string; }' is not comparable to type 'string'.\nsub/b.ts(2,7): error TS2322: Type 'number' is not assignable to type 'string'.\nsub/b.ts(4,26): error TS2322: Type 'string' is not assignable to type 'number'.\n";
    const FILES: [&str; 2] = ["sub/a.ts", "sub/b.ts"];

    #[test]
    fn reports_a_single_error() {
        let one = REAL.lines().next().expect("line");
        let found = parse_tsc(one.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "sub/a.ts");
        assert_eq!(found[0].finding.tool_id, "tsc");
        assert_eq!(found[0].finding.rule_id, "TS2322");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            found[0].finding.start,
            TextPosition {
                line: 1,
                column: 14
            }
        );
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Utf16);
    }

    #[test]
    fn message_chains_attach_to_their_diagnostic() {
        let found = parse_tsc(REAL.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 4);
        assert_eq!(found[1].finding.rule_id, "TS2352");
        assert!(found[1]
            .finding
            .message
            .ends_with("first.\nTypes of property 'b' are incompatible.\nType '{ c: string; }' is not comparable to type 'string'."));
        assert_eq!(found[2].file, "sub/b.ts");
        assert_eq!(
            found[3].finding.start,
            TextPosition {
                line: 4,
                column: 26
            }
        );
    }

    #[test]
    fn crlf_output_parses() {
        let crlf = REAL.replace('\n', "\r\n");
        let found = parse_tsc(crlf.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 4);
        assert_eq!(
            found[0].finding.message,
            "Type 'string' is not assignable to type 'number'."
        );
    }

    #[test]
    fn diagnostics_outside_the_owned_files_are_dropped() {
        let found = parse_tsc(REAL.as_bytes(), Some(2), &["sub/b.ts"]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|found| found.file == "sub/b.ts"));
    }

    #[test]
    fn a_failure_with_no_owned_diagnostic_is_an_error() {
        assert!(parse_tsc(REAL.as_bytes(), Some(2), &["other.ts"]).is_err());
        assert!(parse_tsc(b"", Some(2), &FILES).is_err());
        assert!(parse_tsc(b"", None, &FILES).is_err());
        assert!(parse_tsc(b"", Some(0), &FILES).expect("clean").is_empty());
    }

    #[test]
    fn location_less_and_malformed_diagnostics_fail_closed() {
        for bad in [
            "error TS5083: Cannot read file 'tsconfig.json'.\n",
            "error TS18003: No inputs were found in config file.\n",
            "garbage\n",
            "  orphan continuation\n",
            "sub/a.ts(0,1): error TS1234: msg\n",
            "sub/a.ts(1,0): error TS1234: msg\n",
            "sub/a.ts(x,1): error TS1234: msg\n",
            "sub/a.ts(1,1): warning TS1234: msg\n",
            "sub/a.ts(1,1): error 1234: msg\n",
            "sub/a.ts(1,1): error TS1234: \n",
        ] {
            assert!(
                parse_tsc(bad.as_bytes(), Some(2), &FILES).is_err(),
                "{bad:?}"
            );
        }
        assert!(parse_tsc(&[0xff], Some(2), &FILES).is_err());
    }

    #[test]
    fn splitting_survives_parens_and_separators_in_text() {
        let line = "sub/weird(name).ts(1,2): error TS1234: unexpected token \"): \" here\n";
        let found = parse_tsc(line.as_bytes(), Some(2), &["sub/weird(name).ts"]).expect("parsed");
        assert_eq!(found[0].file, "sub/weird(name).ts");
        assert_eq!(found[0].finding.message, "unexpected token \"): \" here");
    }

    #[test]
    fn the_found_summary_is_ignored_and_the_tool_id_is_a_parameter() {
        let text = format!("{}Found 4 errors in 2 files.\n", REAL);
        let found = parse_tsc_format("vue_tsc", text.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 4);
        assert_eq!(found[0].finding.tool_id, "vue_tsc");
    }
}
