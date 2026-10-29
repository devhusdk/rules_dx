use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, code_name, finding, FileFinding, ParseError};

const TOOL: &str = "sorbet";

/// Marks the end of an error URL: `... https://srb.help/7003`.
const URL_PREFIX: &str = " https://srb.help/";

/// Findings from the stderr of `srb tc` style runs of the `sorbet` binary
/// (`--no-error-sections --color=never`), on exit 0 (clean) or 100 (errors).
///
/// Each error starts with an unindented header `<path>:<line>: <message> https://srb.help/<code>`;
/// the source excerpts and the "Got ... originating from" sections below it are indented and
/// skipped. A run ends with `No errors! Great job.` or `Errors: <count>`; the count must equal
/// the number of header lines, so an error Sorbet printed without a line (a missing file, a
/// crash) fails the run instead of passing. `files` holds the paths of the owned sources as
/// the run spells them; a header for any other file belongs to a dependency and is dropped.
/// The rule is `sorbet/<code>`, `sorbet/error` when the URL is missing; every finding is an
/// error, and Sorbet gives no column, so it is 1. Any other exit code fails the run.
pub fn parse_sorbet(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, output)?;
    match code {
        Some(0) => return Ok(Vec::new()),
        Some(100) => {}
        _ => {
            return Err(ParseError::shape(
                TOOL,
                format!("exit {}: {}", code_name(code), excerpt(text)),
            ))
        }
    }
    let mut headers = 0_usize;
    let mut counted: Option<usize> = None;
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(count) = line.strip_prefix("Errors: ") {
            counted =
                Some(count.trim().parse().map_err(|_| {
                    ParseError::shape(TOOL, format!("malformed error count: {line}"))
                })?);
            continue;
        }
        let Some((path, number, message)) = header(line) else {
            continue;
        };
        headers += 1;
        if !files.contains(&path) {
            continue;
        }
        let (message, rule) = match message.rfind(URL_PREFIX) {
            Some(at) if !message[at + URL_PREFIX.len()..].is_empty() => (
                message[..at].trim_end(),
                format!("sorbet/{}", &message[at + URL_PREFIX.len()..]),
            ),
            _ => (message, "sorbet/error".to_owned()),
        };
        findings.push(finding(
            TOOL,
            path,
            rule,
            message.to_owned(),
            ToolSeverity::Error,
            TextPosition {
                line: number,
                column: 1,
            },
            ColumnEncoding::Scalars,
        ));
    }
    match counted {
        Some(count) if count == headers && headers > 0 => {}
        _ => {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "exit 100 with {headers} error line(s) and count {counted:?}: {}",
                    excerpt(text)
                ),
            ))
        }
    }
    findings.sort_by(|a, b| (&a.file, a.finding.start.line).cmp(&(&b.file, b.finding.start.line)));
    Ok(findings)
}

/// The first lines of output, enough to name why a run failed.
fn excerpt(out: &str) -> String {
    let text = out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("; ");
    if text.is_empty() {
        "no output".to_owned()
    } else {
        text
    }
}

/// Splits `<path>:<line>: <message>`; the path ends at the first `:<digits>: `.
fn header(line: &str) -> Option<(&str, u64, &str)> {
    for (at, _) in line.match_indices(':') {
        let rest = &line[at + 1..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(": ") && at > 0 {
            let number = rest[..digits].parse().ok()?;
            return Some((&line[..at], number, &rest[digits + 2..]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERRORS: &str = "/s/lib/a.rb:10: Unable to resolve constant `Foo` https://srb.help/5002\n    10 |Foo::Bar\n        ^^^\n\n/s/lib/a.rb:6: Expected `String` but found `Integer` for method result type https://srb.help/7005\n     6 |    x\n            ^\n  Expected `String` for result type of method `f`:\n    /s/lib/a.rb:5:\n     5 |  def f(x)\n          ^^^^^^^^\n\n/s/dx-sorbet/dep/d.rb:3: dep https://srb.help/7003\n     3 |x\n\nErrors: 3\n";

    #[test]
    fn sorbet_reports_owned_headers_sorted_and_skips_context() {
        let found = parse_sorbet(ERRORS.as_bytes(), Some(100), &["/s/lib/a.rb"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.file.as_str(),
                    f.finding.start.line,
                    f.finding.rule_id.as_str(),
                    f.finding.message.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                (
                    "/s/lib/a.rb",
                    6,
                    "sorbet/7005",
                    "Expected `String` but found `Integer` for method result type"
                ),
                (
                    "/s/lib/a.rb",
                    10,
                    "sorbet/5002",
                    "Unable to resolve constant `Foo`"
                ),
            ]
        );
        assert!(found
            .iter()
            .all(|f| f.finding.severity == ToolSeverity::Error));
    }

    #[test]
    fn sorbet_clean_and_dependency_only_runs_have_no_findings() {
        assert!(
            parse_sorbet(b"No errors! Great job.\n", Some(0), &["/s/a.rb"])
                .expect("clean")
                .is_empty()
        );
        assert!(parse_sorbet(ERRORS.as_bytes(), Some(100), &["/s/other.rb"])
            .expect("dependency only")
            .is_empty());
    }

    #[test]
    fn sorbet_unrecognised_output_fails_the_run() {
        assert!(parse_sorbet(b"", Some(100), &[]).is_err());
        assert!(parse_sorbet(b"boom\n", Some(1), &[]).is_err());
        assert!(parse_sorbet(b"boom\n", None, &[]).is_err());
        assert!(parse_sorbet(b"Errors: 1\n", Some(100), &["/s/a.rb"]).is_err());
        // An error without a line number is counted but has no header.
        assert!(parse_sorbet(
            b"nonexist.rb: File Not Found https://srb.help/1004\nErrors: 1\n",
            Some(100),
            &["nonexist.rb"]
        )
        .is_err());
        // The summary must match the header lines.
        assert!(parse_sorbet(
            b"/s/a.rb:1: m https://srb.help/1\nErrors: 2\n",
            Some(100),
            &["/s/a.rb"]
        )
        .is_err());
        assert!(parse_sorbet(b"/s/a.rb:1: m\nErrors: x\n", Some(100), &["/s/a.rb"]).is_err());
    }

    #[test]
    fn sorbet_headers_without_a_url_use_the_generic_rule() {
        let found = parse_sorbet(
            b"/s/a b.rb:2: plain message\nErrors: 1\n",
            Some(100),
            &["/s/a b.rb"],
        )
        .expect("parsed");
        assert_eq!(found[0].finding.rule_id, "sorbet/error");
        assert_eq!(found[0].finding.message, "plain message");
    }
}
