use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, code_name, finding, FileFinding, ParseError};

const TOOL: &str = "dialyzer";

/// Findings from `dialyzer --no_indentation --error_location column`, on exit 0 (clean) or 2 (warnings).
///
/// Dialyzer prints every warning to stdout as `file:line:column: message`, followed by indented
/// continuation lines, then the `Unknown functions:` and `Unknown types:` blocks. Beams built with
/// `+deterministic` name files by basename, so `files` holds the checked basenames; a warning in
/// any other file belongs to a dependency and is dropped. Exit 1 is dialyzer failing to analyse.
pub fn parse_dialyzer(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    if !matches!(code, Some(0 | 2)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), failure(out)),
        ));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut unknown: Option<&'static str> = None;
    let mut open = false;
    for line in out.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            open = false;
            continue;
        }
        if !line.starts_with(' ') {
            unknown = match line {
                "Unknown functions:" => Some("unknown_function"),
                "Unknown types:" => Some("unknown_type"),
                _ => None,
            };
            open = false;
            if unknown.is_none() {
                if let Some((file, row, column, message)) = header(line) {
                    open = files.contains(&file);
                    if open {
                        findings.push(located_finding(file, row, column, message));
                    }
                }
            }
            continue;
        }
        if let Some(rule) = unknown {
            if let Some((name, file, row, column)) = reference(line.trim()) {
                if files.contains(&file) {
                    let what = if rule == "unknown_type" {
                        "type"
                    } else {
                        "function"
                    };
                    findings.push(finding(
                        TOOL,
                        file,
                        rule.to_owned(),
                        format!("Unknown {what} {name}"),
                        ToolSeverity::Warning,
                        TextPosition { line: row, column },
                        ColumnEncoding::Scalars,
                    ));
                }
            }
            continue;
        }
        if open {
            if let Some(last) = findings.last_mut() {
                last.finding.message.push(' ');
                last.finding.message.push_str(line.trim());
            }
        }
    }
    findings.sort_by(|a, b| {
        (
            &a.file,
            a.finding.start.line,
            a.finding.start.column,
            &a.finding.rule_id,
        )
            .cmp(&(
                &b.file,
                b.finding.start.line,
                b.finding.start.column,
                &b.finding.rule_id,
            ))
    });
    Ok(findings)
}

/// The run's own account of why it failed, without the banner and the log cache tail.
fn failure(out: &str) -> String {
    let text = out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Proceeding with analysis"))
        .take_while(|line| !line.starts_with("Last messages in the log cache"))
        .collect::<Vec<_>>()
        .join("; ");
    if text.is_empty() {
        "no output".to_owned()
    } else {
        text
    }
}

/// `file:line:column: message` or `file:line: message`, as (file, line, column, message).
fn header(line: &str) -> Option<(&str, u64, u64, &str)> {
    let (file, rest) = line.split_once(':')?;
    if file.is_empty() || file.contains(char::is_whitespace) {
        return None;
    }
    let (row, rest) = rest.split_once(':')?;
    let row: u64 = row.parse().ok()?;
    if let Some((column, message)) = rest.split_once(": ") {
        if let Ok(column) = column.parse::<u64>() {
            return Some((file, row, column.max(1), message.trim()));
        }
    }
    let message = rest.strip_prefix(' ')?;
    Some((file, row, 1, message.trim()))
}

/// `m:f/a (file:line:column)` from an unknown-function or unknown-type block.
fn reference(line: &str) -> Option<(&str, &str, u64, u64)> {
    let (name, place) = line.split_once(" (")?;
    let place = place.strip_suffix(')')?;
    let mut parts = place.rsplitn(3, ':');
    let last = parts.next()?.parse::<u64>().ok()?;
    let middle = parts.next()?;
    match parts.next() {
        Some(file) => Some((name, file, middle.parse().ok()?, last.max(1))),
        None => Some((name, middle, last, 1)),
    }
}

fn located_finding(file: &str, row: u64, column: u64, message: &str) -> FileFinding {
    finding(
        TOOL,
        file,
        rule_for(message).to_owned(),
        message.to_owned(),
        ToolSeverity::Warning,
        TextPosition { line: row, column },
        ColumnEncoding::Scalars,
    )
}

/// The rule a dialyzer warning belongs to, from the wording of its first line.
fn rule_for(message: &str) -> &'static str {
    if message.contains("no local return") || message.contains("will never return") {
        "no_return"
    } else if message.contains("breaks the contract") || message.starts_with("The call ") {
        "call"
    } else if message.contains("type specification") || message.contains("contract") {
        "invalid_contract"
    } else if message.contains("never match")
        || message.contains("never be matched")
        || message.contains("will ever match")
    {
        "pattern_match"
    } else if message.contains("unmatched") {
        "unmatched_return"
    } else {
        "dialyzer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARNINGS: &str = "  Proceeding with analysis...\nbad.erl:3:2: Invalid type specification for function bad:g/1.\n The success typing is bad:g(number()) -> number()\n But the spec is bad:g(integer()) -> atom()\n The return types do not overlap\nbad.erl:5:1: Function f/0 has no local return\nbad.erl:5:18: The pattern 2 can never match the type 1\ndep.erl:9:1: Function x/0 has no local return\nUnknown functions:\n  crypto:hash/2 (bad.erl:6:8)\n  foo:bar/1 (bad.erl:5:31)\n  lists:nope/0 (dep.erl:2:3)\n done in 0m0.08s\ndone (warnings were emitted)\n";

    #[test]
    fn dialyzer_reports_owned_warnings_with_rules_and_continuations() {
        let found = parse_dialyzer(WARNINGS.as_bytes(), Some(2), &["bad.erl"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.finding.start.line,
                    f.finding.start.column,
                    f.finding.rule_id.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                (3, 2, "invalid_contract"),
                (5, 1, "no_return"),
                (5, 18, "pattern_match"),
                (5, 31, "unknown_function"),
                (6, 8, "unknown_function"),
            ]
        );
        assert!(found[0]
            .finding
            .message
            .contains("But the spec is bad:g(integer()) -> atom()"));
        assert_eq!(found[3].finding.message, "Unknown function foo:bar/1");
        assert!(found.iter().all(|f| f.file == "bad.erl"));
    }

    #[test]
    fn dialyzer_reports_unknown_types() {
        let out = "  Proceeding with analysis...\nUnknown types:\n  foo:t/0 (nd.erl:3:14)\n done in 0m0.05s\n";
        let found = parse_dialyzer(out.as_bytes(), Some(2), &["nd.erl"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.rule_id, "unknown_type");
        assert_eq!(found[0].finding.start.column, 14);
    }

    #[test]
    fn dialyzer_reads_line_only_locations_and_call_rules() {
        let out = "a.erl:7: The call lists:map(1,2) breaks the contract (fun(),[any()]) -> [any()]\na.erl:8:3: Expression produces a value of type x, but this value is unmatched\n";
        let found = parse_dialyzer(out.as_bytes(), Some(2), &["a.erl"]).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "call");
        assert_eq!(found[0].finding.start.column, 1);
        assert_eq!(found[1].finding.rule_id, "unmatched_return");
    }

    #[test]
    fn dialyzer_clean_run_has_no_findings() {
        let out = "  Proceeding with analysis...\n done in 0m0.03s\ndone (passed successfully)\n";
        assert!(parse_dialyzer(out.as_bytes(), Some(0), &["a.erl"])
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn dialyzer_failure_names_the_beam() {
        let out = "  Proceeding with analysis...\ndialyzer: Analysis failed with error:\nCould not scan the following file(s):\n  Could not get Core Erlang code for: /s/nd.beam\n\nLast messages in the log cache:\n  Reading files and computing callgraph... \n";
        let err = parse_dialyzer(out.as_bytes(), Some(1), &["nd.erl"]).expect_err("failed");
        assert!(err.to_string().contains("/s/nd.beam"), "{err}");
        assert!(!err.to_string().contains("log cache"), "{err}");
        assert!(parse_dialyzer(b"", None, &[]).is_err());
    }
}
