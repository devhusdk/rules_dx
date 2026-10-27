use super::{as_text, code_name, hunk_counts, known, path_row_col, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity, LINE_END};

const TOOL: &str = "scalafix";
const TEST_ERROR: i32 = 32;
const LINT_ERROR: i32 = 64;
const SEVERITIES: [(&str, ToolSeverity); 3] = [
    (": error: [", ToolSeverity::Error),
    (": warning: [", ToolSeverity::Warning),
    (": info: [", ToolSeverity::Info),
];
const EXIT_BITS: [(i32, &str); 6] = [
    (1, "unexpected error"),
    (2, "parse error"),
    (4, "command line error"),
    (8, "SemanticDB not found"),
    (16, "stale SemanticDB"),
    (128, "no files to check"),
];

fn failure(code: i32, stdout: &str, stderr: &[u8]) -> ParseError {
    let named: Vec<&str> = EXIT_BITS
        .iter()
        .filter(|(bit, _)| code & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    let stderr = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(5)
        .collect();
    let mut detail = format!("exit {code} ({})", named.join(", "));
    if !lines.is_empty() {
        detail.push_str(": ");
        detail.push_str(&lines.join("; "));
    }
    ParseError::shape(TOOL, detail)
}

struct Lint<'a> {
    path: &'a str,
    line: u64,
    column: u64,
    severity: ToolSeverity,
    rule: &'a str,
    message: &'a str,
}

fn lint_header(line: &str) -> Option<Lint<'_>> {
    let (start, marker, severity) = SEVERITIES
        .iter()
        .filter_map(|(marker, severity)| line.find(marker).map(|at| (at, *marker, *severity)))
        .min_by_key(|(at, _, _)| *at)?;
    let (rule, message) = line[start + marker.len()..].split_once(']')?;
    let (path, line, column) = path_row_col(&line[..start])?;
    Some(Lint {
        path,
        line,
        column,
        severity,
        rule,
        message: message.trim(),
    })
}

fn is_diff_start(lines: &[&str], at: usize) -> bool {
    lines[at].starts_with("--- ")
        && lines
            .get(at + 1)
            .is_some_and(|next| next.starts_with("+++ "))
}

fn caret_width(line: &str) -> Option<u64> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c == '^') {
        return None;
    }
    Some(trimmed.chars().count() as u64)
}

struct Hunk {
    first: u64,
    last: u64,
}

fn hunk_start(header: &str) -> Option<u64> {
    let rest = header.strip_prefix("@@ -")?;
    let (old, _) = rest.split_once(" +")?;
    old.split_once(',')
        .map_or(old, |(start, _)| start)
        .parse()
        .ok()
}

fn hunk_span(header: &str, body: &[&str]) -> Result<Hunk, ParseError> {
    let mut old = hunk_start(header)
        .ok_or_else(|| ParseError::shape(TOOL, format!("malformed hunk: {header}")))?;
    let mut first = None;
    let mut last = 1;
    for line in body {
        match line.chars().next() {
            Some('-') => {
                first.get_or_insert(old);
                last = last.max(old);
                old += 1;
            }
            Some('+') => {
                let anchor = old.saturating_sub(1).max(1);
                first.get_or_insert(anchor);
                last = last.max(anchor);
            }
            Some('\\') => {}
            _ => old += 1,
        }
    }
    let first =
        first.ok_or_else(|| ParseError::shape(TOOL, format!("hunk changes nothing: {header}")))?;
    Ok(Hunk { first, last })
}

fn patch_finding(file: &str, hunk: &Hunk) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: "Patch".to_owned(),
            message: "scalafix would rewrite this code".to_owned(),
            severity: ToolSeverity::Error,
            start: TextPosition {
                line: hunk.first,
                column: 1,
            },
            end: Some(TextPosition {
                line: hunk.last,
                column: LINE_END,
            }),
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

pub fn parse_scalafix(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let Some(code) = code else {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} (killed)", code_name(None)),
        ));
    };
    if code & !(TEST_ERROR | LINT_ERROR) != 0 {
        return Err(failure(code, text, stderr));
    }
    if text.contains("No rules requested to run") {
        return Err(ParseError::shape(
            TOOL,
            "no rules requested; set rules in the Scalafix config",
        ));
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();
    let mut patches = 0;
    let mut errors = 0;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(lint) = lint_header(line) {
            let file = known(TOOL, files, lint.path)?;
            let mut end = None;
            let mut message = lint.message.to_owned();
            let mut scan = index;
            while scan < lines.len()
                && lint_header(lines[scan]).is_none()
                && !is_diff_start(&lines, scan)
            {
                if scan > index {
                    if let Some(width) = caret_width(lines[scan]) {
                        end = Some(TextPosition {
                            line: lint.line,
                            column: lint.column + width,
                        });
                        for extra in &lines[index..scan - 1] {
                            message.push('\n');
                            message.push_str(extra.trim());
                        }
                        index = scan + 1;
                        break;
                    }
                }
                scan += 1;
            }
            if lint.line == 0 || lint.column == 0 || lint.rule.is_empty() {
                return Err(ParseError::shape(TOOL, format!("malformed lint: {line}")));
            }
            if lint.severity == ToolSeverity::Error {
                errors += 1;
            }
            findings.push(FileFinding {
                file: file.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: lint.rule.to_owned(),
                    message,
                    severity: lint.severity,
                    start: TextPosition {
                        line: lint.line,
                        column: lint.column,
                    },
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
            continue;
        }
        if let (Some(path), Some(next)) = (line.strip_prefix("--- "), lines.get(index)) {
            if next.starts_with("+++ ") {
                let file = known(TOOL, files, path.trim())?;
                index += 1;
                while index < lines.len() && lines[index].starts_with("@@ ") {
                    let header = lines[index];
                    let (old, new) = hunk_counts(header).ok_or_else(|| {
                        ParseError::shape(TOOL, format!("malformed hunk: {header}"))
                    })?;
                    index += 1;
                    let begin = index;
                    let (mut seen_old, mut seen_new) = (0, 0);
                    while index < lines.len() && (seen_old < old || seen_new < new) {
                        match lines[index].chars().next() {
                            Some('-') => seen_old += 1,
                            Some('+') => seen_new += 1,
                            Some('\\') => {}
                            _ => {
                                seen_old += 1;
                                seen_new += 1;
                            }
                        }
                        index += 1;
                    }
                    if seen_old != old || seen_new != new {
                        return Err(ParseError::shape(TOOL, format!("truncated hunk: {header}")));
                    }
                    while index < lines.len() && lines[index].starts_with('\\') {
                        index += 1;
                    }
                    let hunk = hunk_span(header, &lines[begin..index])?;
                    findings.push(patch_finding(file, &hunk));
                    patches += 1;
                }
                continue;
            }
        }
        return Err(ParseError::shape(
            TOOL,
            format!("unrecognized line: {}", line.trim()),
        ));
    }
    if (code & TEST_ERROR != 0) != (patches > 0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {code} disagrees with {patches} reported rewrites"),
        ));
    }
    if (code & LINT_ERROR != 0) != (errors > 0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {code} disagrees with {errors} reported lint errors"),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/app/V.scala";

    const LINT: &str = "/s/app/V.scala:3:3: error: [DisableSyntax.var] mutable state should be avoided\n  var x = 1\n  ^^^\n/s/app/V.scala:4:19: warning: [DisableSyntax.null] null should be avoided, consider using Option instead\n  val y: String = null\n                  ^^^^\n";

    const DIFF: &str = "--- /s/app/V.scala\n+++ <expected fix>\n@@ -3,5 +3,5 @@\n \n object Helper {\n   def suffix(): String = \" world\"\n-  def n = List(1)\n+  def n: List[Int] = List(1)\n }\n";

    fn parse(out: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_scalafix(out.as_bytes(), b"", Some(code), &[FILE])
    }

    #[test]
    fn lint_errors_and_warnings_carry_rule_severity_and_span() {
        let findings = parse(LINT, 64).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.rule_id, "DisableSyntax.var");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 3, column: 3 }
        );
        assert_eq!(
            findings[0].finding.end,
            Some(TextPosition { line: 3, column: 6 })
        );
        assert_eq!(findings[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[1].finding.message,
            "null should be avoided, consider using Option instead"
        );
    }

    #[test]
    fn multi_line_lint_messages_keep_every_line_and_the_span() {
        let out = "/s/app/V.scala:3:3: error: [DisableSyntax.regex] first line\n  second line\n  third line\n  var x = 1\n  ^^^\n/s/app/V.scala:4:3: warning: [DisableSyntax.null] null\n  val y = null\n  ^^^^\n";
        let findings = parse(out, 64).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(
            findings[0].finding.message,
            "first line\nsecond line\nthird line"
        );
        assert_eq!(
            findings[0].finding.end,
            Some(TextPosition { line: 3, column: 6 })
        );
        assert_eq!(findings[1].finding.message, "null");
    }

    #[test]
    fn warnings_alone_pass_with_exit_zero() {
        let out = "/s/app/V.scala:4:19: info: [DisableSyntax.null] null\n  val y = null\n          ^^^^\n";
        let findings = parse(out, 0).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert!(parse("", 0).expect("clean").is_empty());
    }

    #[test]
    fn rewrites_report_the_changed_lines() {
        let findings = parse(DIFF, 32).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "Patch");
        assert_eq!(findings[0].finding.start.line, 6);
        assert_eq!(findings[0].finding.end.map(|end| end.line), Some(6));
    }

    #[test]
    fn rewrites_and_lint_errors_can_share_one_run() {
        let both = format!("{LINT}{DIFF}");
        let findings = parse(&both, 96).expect("parsed");
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn runtime_failures_are_never_clean() {
        for (code, name) in [
            (1, "unexpected error"),
            (2, "parse error"),
            (4, "command line error"),
            (8, "SemanticDB not found"),
            (16, "stale SemanticDB"),
            (128, "no files to check"),
        ] {
            let err =
                parse("error: SemanticDB not found: app/V.scala\n", code).expect_err("failure");
            assert!(err.to_string().contains(name), "{err}");
            assert!(err
                .to_string()
                .contains("SemanticDB not found: app/V.scala"));
        }
        assert!(parse("", 8 | 32).is_err());
        assert!(parse_scalafix(b"", b"", None, &[FILE]).is_err());
    }

    #[test]
    fn stderr_text_names_the_failure() {
        let err = parse_scalafix(b"", b"Exception in thread main\n", Some(1), &[FILE])
            .expect_err("failure");
        assert!(err.to_string().contains("Exception in thread main"));
    }

    #[test]
    fn an_empty_rule_set_is_not_a_clean_run() {
        let err = parse("No rules requested to run\n", 0).expect_err("no rules");
        assert!(err.to_string().contains("set rules"));
    }

    #[test]
    fn inconsistent_output_is_rejected() {
        assert!(parse(DIFF, 0).is_err());
        assert!(parse("", 32).is_err());
        assert!(parse(LINT, 0).is_err());
        assert!(parse("", 64).is_err());
        assert!(parse(DIFF, 64).is_err());
    }

    #[test]
    fn unknown_files_and_noise_are_rejected() {
        let other = LINT.replace("/s/app/V.scala", "/s/app/Other.scala");
        assert!(parse(&other, 64).is_err());
        assert!(parse("surprise\n", 0).is_err());
        assert!(parse(
            "--- /s/app/V.scala\n+++ <expected fix>\n@@ -1 +1 @@\n-a\n",
            32
        )
        .is_err());
        assert!(parse_scalafix(&[0xff], b"", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn insertions_anchor_on_the_previous_line() {
        let out = "--- /s/app/V.scala\n+++ <expected fix>\n@@ -2,2 +2,3 @@\n a\n+inserted\n b\n";
        let findings = parse(out, 32).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 2);
    }
}
