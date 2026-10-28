use crate::{ColumnEncoding, ToolSeverity};

use super::sarif::{read_sarif, sarif_findings};
use super::{code_name, FileFinding, ParseError};

const TOOL: &str = "ansible_lint";
const ALWAYS_ERROR: [&str; 4] = [
    "syntax-check",
    "load-failure",
    "parser-error",
    "internal-error",
];

fn last_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no stderr")
        .trim()
        .to_owned()
}

fn forced_error(rule: &str) -> bool {
    ALWAYS_ERROR
        .iter()
        .any(|prefix| rule == *prefix || rule.starts_with(&format!("{prefix}[")))
}

/// Parses `ansible-lint -f sarif`; only exit 0 (clean) and 2 (violations) are accepted.
pub fn parse_ansible_lint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0) | Some(2)) {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("exit {}: {}", code_name(code), last_line(stderr)),
        });
    }
    let log = read_sarif(TOOL, stdout)?;
    let mut findings = sarif_findings(TOOL, &log, code, files, ColumnEncoding::Scalars)?;
    for item in &mut findings {
        if forced_error(&item.finding.rule_id) {
            item.finding.severity = ToolSeverity::Error;
        }
    }
    let errors = findings
        .iter()
        .any(|item| item.finding.severity == ToolSeverity::Error);
    if code == Some(2) && !errors {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit 2 without an error result: {}", last_line(stderr)),
        });
    }
    if code == Some(0) && errors {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "exit 0 with an error result".to_owned(),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(results: &str) -> String {
        format!(
            r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"ansible-lint"}}}},"originalUriBaseIds":{{"SRCROOT":{{"uri":"file:///scratch/"}}}},"results":[{results}]}}]}}"#
        )
    }

    fn result(rule: &str, level: &str, path: &str) -> String {
        format!(
            r#"{{"ruleId":"{rule}","level":"{level}","message":{{"text":"msg"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{path}","uriBaseId":"SRCROOT"}},"region":{{"startLine":3}}}}}}]}}"#
        )
    }

    const FILES: [&str; 2] = ["/scratch/a.yml", "/scratch/b.yml"];

    #[test]
    fn clean_run() {
        let out = parse_ansible_lint(log("").as_bytes(), b"", Some(0), &FILES).expect("clean");
        assert!(out.is_empty());
    }

    #[test]
    fn dirty_multi_file() {
        let body = format!(
            "{},{}",
            result("name[missing]", "error", "a.yml"),
            result("fqcn[action-core]", "error", "b.yml")
        );
        let out = parse_ansible_lint(log(&body).as_bytes(), b"", Some(2), &FILES).expect("dirty");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].file, "/scratch/a.yml");
        assert_eq!(out[1].finding.rule_id, "fqcn[action-core]");
    }

    #[test]
    fn syntax_check_is_error() {
        let body = format!(
            "{},{}",
            result("syntax-check[unknown-module]", "warning", "a.yml"),
            result("name[play]", "warning", "b.yml")
        );
        let out = parse_ansible_lint(log(&body).as_bytes(), b"", Some(2), &FILES).expect("syntax");
        assert_eq!(out[0].finding.severity, ToolSeverity::Error);
        assert_eq!(out[1].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn warning_only_exit_zero() {
        let body = result("name[play]", "warning", "a.yml");
        let out = parse_ansible_lint(log(&body).as_bytes(), b"", Some(0), &FILES).expect("warn");
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn exit_two_without_results_fails() {
        assert!(parse_ansible_lint(log("").as_bytes(), b"", Some(2), &FILES).is_err());
    }

    #[test]
    fn exit_zero_with_error_fails() {
        let body = result("name[play]", "error", "a.yml");
        assert!(parse_ansible_lint(log(&body).as_bytes(), b"", Some(0), &FILES).is_err());
    }

    #[test]
    fn other_exit_codes_surface_stderr() {
        for code in [3, 4, 5, 69] {
            let err = parse_ansible_lint(b"", b"warn\nbad config\n", Some(code), &FILES)
                .expect_err("exit");
            let text = err.to_string();
            assert!(text.contains(&format!("exit {code}")), "{text}");
            assert!(text.contains("bad config"), "{text}");
        }
        assert!(parse_ansible_lint(b"", b"", None, &FILES).is_err());
    }

    #[test]
    fn malformed_and_empty_stdout_fail() {
        assert!(parse_ansible_lint(b"{", b"", Some(0), &FILES).is_err());
        assert!(parse_ansible_lint(b"", b"", Some(0), &FILES).is_err());
        assert!(parse_ansible_lint(b"", b"", Some(2), &FILES).is_err());
    }

    #[test]
    fn path_outside_files_fails() {
        let body = result("name[play]", "error", "other.yml");
        assert!(parse_ansible_lint(log(&body).as_bytes(), b"", Some(2), &FILES).is_err());
    }
}
