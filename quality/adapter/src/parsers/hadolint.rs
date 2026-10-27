use super::{as_text, code_name, known_spelled, point, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "hadolint";

/// What hadolint 2.15.1 prints to stderr before it falls back to its defaults on a bad config.
const CONFIG_ERROR: &str = "Error parsing your config file";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn position(value: &serde_json::Value, key: &str) -> Result<u64, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|number| number.max(1))
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

/// Parses `hadolint -f json`: stdout is one JSON array of findings, whatever the exit code says.
pub fn parse_hadolint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.contains(CONFIG_ERROR) {
        let first = stderr.trim().lines().next().unwrap_or_default();
        return Err(ParseError::shape(
            TOOL,
            format!(
                "the hadolint config is invalid and hadolint fell back to its defaults: {first}"
            ),
        ));
    }
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), stderr.trim()),
        ));
    }
    let report = as_text(TOOL, stdout)?.trim();
    let entries: Vec<serde_json::Value> = if report.starts_with('[') {
        serde_json::from_str(report).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!(
                "stdout must be one JSON array ({err}); remove `format` and `output` from the hadolint config"
            ),
        })?
    } else {
        return Err(ParseError::Json {
            tool: TOOL,
            detail: format!(
                "stdout must be one JSON array, got {:?}{}; remove `format` and `output` from the hadolint config",
                report.chars().take(80).collect::<String>(),
                if stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(" (stderr: {})", stderr.trim())
                },
            ),
        });
    };
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        let checked = known_spelled(TOOL, files, text(entry, "file")?)?;
        let severity = match text(entry, "level")? {
            "error" => ToolSeverity::Error,
            "warning" => ToolSeverity::Warning,
            "info" | "style" => ToolSeverity::Info,
            other => return Err(ParseError::shape(TOOL, format!("unknown level {other:?}"))),
        };
        let rule_id = text(entry, "code")?;
        if rule_id.is_empty() {
            return Err(ParseError::shape(TOOL, "missing code"));
        }
        let message = text(entry, "message")?.trim();
        if message.is_empty() {
            return Err(ParseError::shape(TOOL, "missing message"));
        }
        let (start, end) = point(position(entry, "line")?, position(entry, "column")?);
        findings.push(FileFinding {
            file: checked.to_owned(),
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
        });
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"[{"code":"DL4006","column":1,"file":"Dockerfile","level":"warning","line":2,"message":"Set the SHELL option -o pipefail before RUN with a pipe in it."},{"code":"SC2086","column":1,"file":"Dockerfile","level":"info","line":2,"message":"Double quote to prevent globbing and word splitting."}]"#;
    const SYNTAX: &str = r#"[{"code":"DL1000","column":4,"file":"sub2/Dockerfile","level":"error","line":2,"message":"missing whitespace"}]"#;
    const CONFIG_STDERR: &str = "\"Error parsing your config file in  '/scratch/.hadolint.yaml':\\nIt should contain one of the keys 'override', 'ignored'\\nor 'trustedRegistries'.\"\n";

    #[test]
    fn hadolint_clean_run_is_clean() {
        let clean = parse_hadolint(b"[]", b"", Some(0), &["Dockerfile"]).expect("parsed");
        assert!(clean.is_empty());
        let spaced = parse_hadolint(b"\n[]\n", b"", Some(0), &["Dockerfile"]).expect("parsed");
        assert!(spaced.is_empty());
    }

    #[test]
    fn hadolint_reports_dl_and_sc_rules() {
        let findings =
            parse_hadolint(DIRTY.as_bytes(), b"", Some(1), &["Dockerfile"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Dockerfile");
        assert_eq!(findings[0].finding.tool_id, "hadolint");
        assert_eq!(findings[0].finding.rule_id, "DL4006");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[1].finding.rule_id, "SC2086");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Info);
    }

    #[test]
    fn hadolint_parse_errors_are_error_findings() {
        let findings =
            parse_hadolint(SYNTAX.as_bytes(), b"", Some(1), &["sub2/Dockerfile"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "DL1000");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 4);
    }

    #[test]
    fn hadolint_style_is_info() {
        let style = br#"[{"code":"DL3059","column":1,"file":"Dockerfile","level":"style","line":3,"message":"Multiple consecutive RUN instructions."}]"#;
        let findings = parse_hadolint(style, b"", Some(1), &["Dockerfile"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        let unknown = br#"[{"code":"DL3059","column":1,"file":"Dockerfile","level":"fatal","line":3,"message":"x"}]"#;
        assert!(parse_hadolint(unknown, b"", Some(1), &["Dockerfile"]).is_err());
    }

    #[test]
    fn hadolint_config_error_is_never_clean() {
        let err = parse_hadolint(b"[]", CONFIG_STDERR.as_bytes(), Some(0), &["Dockerfile"])
            .expect_err("config error");
        assert!(err.to_string().contains("config"), "{err}");
        assert!(parse_hadolint(
            DIRTY.as_bytes(),
            CONFIG_STDERR.as_bytes(),
            Some(1),
            &["Dockerfile"]
        )
        .is_err());
    }

    #[test]
    fn hadolint_stdout_must_be_one_json_array() {
        let tty = format!(
            "Dockerfile:2 DL4006 warning: Set the SHELL option -o pipefail before RUN with a pipe in it.\n{DIRTY}"
        );
        let err =
            parse_hadolint(tty.as_bytes(), b"", Some(1), &["Dockerfile"]).expect_err("tty prefix");
        assert!(err.to_string().contains("remove `format`"), "{err}");
        let trailing = format!("{DIRTY}\nDockerfile:2 DL4006 warning: x\n");
        assert!(parse_hadolint(trailing.as_bytes(), b"", Some(1), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(br#""[]""#, b"", Some(0), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(br#"{"code":"DL1000"}"#, b"", Some(1), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(&[0xff], b"", Some(0), &["Dockerfile"]).is_err());
    }

    #[test]
    fn hadolint_empty_output_with_failure_is_an_error() {
        assert!(parse_hadolint(b"", b"", Some(1), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(b"", b"", Some(0), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(b"[]", b"", Some(1), &["Dockerfile"]).is_err());
        let missing =
            b"hadolint: missing: withBinaryFile: does not exist (No such file or directory)\n";
        assert!(parse_hadolint(b"", missing, Some(1), &["missing"]).is_err());
        assert!(parse_hadolint(
            b"",
            b"Please provide a Dockerfile\n",
            Some(1),
            &["Dockerfile"]
        )
        .is_err());
        assert!(parse_hadolint(DIRTY.as_bytes(), b"", Some(2), &["Dockerfile"]).is_err());
        assert!(parse_hadolint(DIRTY.as_bytes(), b"", None, &["Dockerfile"]).is_err());
    }

    #[test]
    fn hadolint_exit_zero_with_findings_still_reports_them() {
        let findings =
            parse_hadolint(DIRTY.as_bytes(), b"", Some(0), &["Dockerfile"]).expect("no-fail");
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn hadolint_reads_backslash_paths() {
        let report = br#"[{"code":"DL3006","column":1,"file":"sub\\Dockerfile","level":"warning","line":1,"message":"Always tag the version of an image explicitly"}]"#;
        let findings = parse_hadolint(report, b"", Some(1), &["sub/Dockerfile"]).expect("parsed");
        assert_eq!(findings[0].file, "sub/Dockerfile");
    }

    #[test]
    fn hadolint_rejects_unknown_files() {
        let err = parse_hadolint(DIRTY.as_bytes(), b"", Some(1), &["other/Dockerfile"])
            .expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
