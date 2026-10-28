use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, code_name, finding, require_findings, FileFinding, ParseError};

const TOOL: &str = "phpstan";

/// Findings from `phpstan analyse --error-format=json`, on exit 0 (clean) or 1 (findings).
///
/// PHPStan keys `files` by the path it analysed, so `files` holds the paths of the owned
/// sources as PHPStan reports them; an entry for any other file belongs to a dependency and is
/// dropped. A clean run prints `"files": []`, an empty array rather than an object. The
/// top-level `errors` array carries errors that are not tied to a file (a bad config, an
/// unreadable path) and is never clean, whatever the exit code says. Any other exit code (255
/// for a crash, none for a signal) fails the run. The rule is the error's `identifier`, `error`
/// when PHPStan gave none, and a `tip` joins the message.
pub fn parse_phpstan(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        let out = as_text(TOOL, stdout)?;
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), excerpt(out)),
        ));
    }
    let text = as_text(TOOL, stdout)?;
    let report: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|err| ParseError::shape(TOOL, format!("output is not JSON: {err}")))?;
    let general = report
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "report without errors".to_owned()))?;
    if let Some(first) = general.first() {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "{} general error(s): {}",
                general.len(),
                first.as_str().unwrap_or("not text")
            ),
        ));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    match report.get("files") {
        Some(serde_json::Value::Object(by_file)) => {
            for (path, entry) in by_file {
                if !files.contains(&path.as_str()) {
                    continue;
                }
                let messages = entry
                    .get("messages")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| ParseError::shape(TOOL, format!("{path} without messages")))?;
                for message in messages {
                    findings.push(one(path, message)?);
                }
            }
        }
        Some(serde_json::Value::Array(empty)) if empty.is_empty() => {}
        _ => return Err(ParseError::shape(TOOL, "report without files".to_owned())),
    }
    findings.sort_by(|a, b| {
        (&a.file, a.finding.start.line, &a.finding.rule_id).cmp(&(
            &b.file,
            b.finding.start.line,
            &b.finding.rule_id,
        ))
    });
    require_findings(TOOL, &findings, code)?;
    if code == Some(0) && !findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 0 with diagnostics".to_owned(),
        ));
    }
    Ok(findings)
}

fn one(path: &str, message: &serde_json::Value) -> Result<FileFinding, ParseError> {
    let text = message
        .get("message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} message is not text")))?;
    // A line-less error (a file-level one) is reported at the top of the file.
    let line = message
        .get("line")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let rule = message
        .get("identifier")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .map_or_else(|| "phpstan/error".to_owned(), |id| format!("phpstan/{id}"));
    let mut full = text.to_owned();
    if let Some(tip) = message
        .get("tip")
        .and_then(serde_json::Value::as_str)
        .filter(|tip| !tip.is_empty())
    {
        full.push(' ');
        full.push_str(tip);
    }
    Ok(finding(
        TOOL,
        path,
        rule,
        full,
        ToolSeverity::Error,
        TextPosition { line, column: 1 },
        ColumnEncoding::Scalars,
    ))
}

/// The first lines of output, enough to name why a run crashed.
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

#[cfg(test)]
mod tests {
    use super::*;

    const FINDINGS: &str = r#"{"totals":{"errors":0,"file_errors":3},"files":{"/s/a.php":{"errors":2,"messages":[{"message":"Method A::f() should return int but returns string.","line":7,"ignorable":true,"identifier":"return.type"},{"message":"Call to an undefined method A::g().","line":4,"ignorable":true,"tip":"Did you mean f?"}]},"/s/dep.php":{"errors":1,"messages":[{"message":"dep","line":1,"ignorable":true,"identifier":"x.y"}]}},"errors":[]}"#;

    #[test]
    fn phpstan_reports_owned_findings_with_rules_and_tips() {
        let found = parse_phpstan(FINDINGS.as_bytes(), Some(1), &["/s/a.php"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.file.as_str(),
                    f.finding.start.line,
                    f.finding.rule_id.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                ("/s/a.php", 4, "phpstan/error"),
                ("/s/a.php", 7, "phpstan/return.type"),
            ]
        );
        assert_eq!(
            found[0].finding.message,
            "Call to an undefined method A::g(). Did you mean f?"
        );
    }

    #[test]
    fn phpstan_clean_run_has_no_findings() {
        let out = br#"{"totals":{"errors":0,"file_errors":0},"files":[],"errors":[]}"#;
        assert!(parse_phpstan(out, Some(0), &["/s/a.php"])
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn phpstan_findings_in_other_files_alone_are_an_error_on_exit_one() {
        let err =
            parse_phpstan(FINDINGS.as_bytes(), Some(1), &["/s/other.php"]).expect_err("no owned");
        assert!(err.to_string().contains("no diagnostics"), "{err}");
    }

    #[test]
    fn phpstan_general_errors_fail_the_run() {
        let out = br#"{"totals":{"errors":1,"file_errors":0},"files":[],"errors":["Path /s/x.php does not exist"]}"#;
        let err = parse_phpstan(out, Some(1), &["/s/a.php"]).expect_err("general");
        assert!(err.to_string().contains("does not exist"), "{err}");
    }

    #[test]
    fn phpstan_crash_signal_and_garbage_fail_the_run() {
        let err =
            parse_phpstan(b"Fatal error: out of memory\n", Some(255), &[]).expect_err("crash");
        assert!(err.to_string().contains("255"), "{err}");
        assert!(parse_phpstan(b"", None, &[]).is_err());
        assert!(parse_phpstan(b"not json", Some(1), &[]).is_err());
        assert!(parse_phpstan(b"{}", Some(0), &[]).is_err());
    }
}
