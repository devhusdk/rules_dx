use super::{as_text, code_name, known_spelled, point, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "checkmake";

/// Go `%q` for the config path checkmake logs when it loads the file it was asked to load.
fn go_quoted(path: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in path.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn finding(
    file: &str,
    rule_id: &str,
    message: &str,
    severity: ToolSeverity,
    line: u64,
) -> FileFinding {
    let (start, end) = point(line.max(1), 1);
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule_id.to_owned(),
            message: message.to_owned(),
            severity,
            start,
            end,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

/// Parses `checkmake --debug --output json --config <config> <files>`.
///
/// checkmake exits 1 for findings and for failures alike and silently falls back to its
/// defaults when the config is unreadable, so this never decides from the exit code alone:
/// the run only counts when stderr confirms it loaded exactly `config`. `--debug` is what
/// makes checkmake log that. Pass `config: None` only for already-validated delegated JSON.
pub fn parse_checkmake(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    config: Option<&str>,
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    if let Some(config) = config {
        if stderr.contains("Unable to parse config file")
            || stderr.contains("error accessing config file")
        {
            let first = stderr
                .lines()
                .find(|line| line.contains("config file"))
                .unwrap_or_default();
            return Err(ParseError::shape(
                TOOL,
                format!("the checkmake config is invalid and checkmake fell back to its defaults: {first}"),
            ));
        }
        let wanted = format!("Using configuration file: {}", go_quoted(config));
        if !stderr.lines().any(|line| line.ends_with(&wanted)) {
            return Err(ParseError::shape(
                TOOL,
                format!(
                    "checkmake did not confirm loading the config {config:?} (stderr: {})",
                    stderr.trim().chars().take(200).collect::<String>()
                ),
            ));
        }
    }
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), stderr.trim()),
        ));
    }
    if let Some(rest) = stderr.split("failed to parse \"").nth(1) {
        let name = rest.split('"').next().unwrap_or_default();
        let file = known_spelled(TOOL, files, name)?;
        let detail = rest
            .split_once("\": ")
            .map_or("", |(_, tail)| tail.lines().next().unwrap_or(""));
        return Ok(vec![finding(
            file,
            "parse-error",
            format!("checkmake could not parse this file: {detail}").trim_end_matches([':', ' ']),
            ToolSeverity::Error,
            1,
        )]);
    }
    let report = as_text(TOOL, stdout)?.trim();
    if report.is_empty() {
        return if code == Some(0) {
            Ok(Vec::new())
        } else {
            Err(ParseError::shape(
                TOOL,
                format!("exit 1 with no diagnostics (stderr: {})", stderr.trim()),
            ))
        };
    }
    if !report.starts_with('[') {
        return Err(ParseError::Json {
            tool: TOOL,
            detail: format!(
                "stdout must be one JSON array, got {:?}",
                report.chars().take(80).collect::<String>()
            ),
        });
    }
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(report).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        let file = known_spelled(TOOL, files, text(entry, "file_name")?)?;
        let rule_id = text(entry, "rule")?;
        let message = text(entry, "violation")?.trim();
        if rule_id.is_empty() || message.is_empty() {
            return Err(ParseError::shape(TOOL, "missing rule or violation"));
        }
        let line = entry
            .get("line_number")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| ParseError::shape(TOOL, "missing line_number"))?;
        findings.push(finding(
            file,
            rule_id,
            message,
            ToolSeverity::Warning,
            u64::try_from(line).unwrap_or(0),
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "/scratch/dx-checkmake-default/checkmake.ini";
    const DIRTY: &str = r#"[{"rule":"minphony","violation":"Required target \"all\" must be declared PHONY.","file_name":"Makefile","line_number":1},{"rule":"minphony","violation":"Required target \"test\" is missing from the Makefile.","file_name":"sub/GNUmakefile","line_number":-1}]"#;

    fn stderr(extra: &str) -> Vec<u8> {
        format!("2026/10/28 00:28:58 Using configuration file: \"{CFG}\"\n2026/10/28 00:28:58 Parsed configuration:\n\n{extra}")
            .into_bytes()
    }

    fn run(stdout: &[u8], err: &[u8], code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_checkmake(
            stdout,
            err,
            code,
            &["Makefile", "sub/GNUmakefile", "bad.mk"],
            Some(CFG),
        )
    }

    #[test]
    fn checkmake_clean_run_is_clean() {
        assert!(run(b"", &stderr(""), Some(0)).expect("clean").is_empty());
    }

    #[test]
    fn checkmake_reports_findings_with_clamped_lines() {
        let findings = run(
            DIRTY.as_bytes(),
            &stderr("Error: violations found (2)\n"),
            Some(1),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Makefile");
        assert_eq!(findings[0].finding.rule_id, "minphony");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[1].file, "sub/GNUmakefile");
        assert_eq!(findings[1].finding.start.line, 1);
    }

    #[test]
    fn checkmake_missing_or_other_config_is_never_clean() {
        let fallback = b"2026/10/28 00:28:58 Unable to parse config file \"checkmake.ini\", running with defaults\n";
        assert!(run(b"", fallback, Some(0)).is_err());
        assert!(run(b"", b"", Some(0)).is_err());
        let other = b"2026/10/28 00:28:58 Using configuration file: \"/home/u/checkmake.ini\"\n";
        assert!(run(b"", other, Some(0)).is_err());
        let broken =
            stderr("2026/10/28 Unable to parse config file \"x\", running with defaults\n");
        assert!(run(b"", &broken, Some(0)).is_err());
        let access = stderr("error accessing config file\n");
        assert!(run(b"", &access, Some(0)).is_err());
    }

    #[test]
    fn checkmake_parse_failure_is_an_error_on_that_file() {
        let err = stderr(
            "Error: failed to parse \"bad.mk\": Error opening the provided filepath 'bad.mk'\n",
        );
        let findings = run(b"", &err, Some(1)).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "bad.mk");
        assert_eq!(findings[0].finding.rule_id, "parse-error");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn checkmake_fails_closed_on_odd_output() {
        assert!(run(b"", &stderr(""), Some(1)).is_err());
        assert!(run(DIRTY.as_bytes(), &stderr(""), Some(2)).is_err());
        assert!(run(DIRTY.as_bytes(), &stderr(""), None).is_err());
        assert!(run(b"Makefile:1 minphony", &stderr(""), Some(1)).is_err());
        assert!(run(b"not json", &stderr(""), Some(0)).is_err());
        assert!(run(b"[]", &stderr(""), Some(1)).is_err());
        assert!(run(&[0xff], &stderr(""), Some(0)).is_err());
        assert!(run(br#"{"rule":"x"}"#, &stderr(""), Some(1)).is_err());
    }

    #[test]
    fn checkmake_rejects_unknown_files() {
        let err = parse_checkmake(
            DIRTY.as_bytes(),
            &stderr(""),
            Some(1),
            &["other"],
            Some(CFG),
        )
        .expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }

    #[test]
    fn checkmake_quotes_windows_config_paths_like_go() {
        let err = br#"2026/10/28 Using configuration file: "C:\\s\\checkmake.ini""#;
        let findings = parse_checkmake(
            b"",
            err,
            Some(0),
            &["Makefile"],
            Some(r"C:\s\checkmake.ini"),
        );
        assert!(findings.expect("clean").is_empty());
    }
}
