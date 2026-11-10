use super::{FileFinding, ParseError, as_text, code_name, known_spelled, point, require_findings};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "actionlint";

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

/// Parses `actionlint -format '{{json .}}'`: one JSON array, or `null` when nothing was found.
/// Exit 1 means findings; 2 and 3 are usage and fatal errors.
pub fn parse_actionlint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stderr).trim()
            ),
        ));
    }
    let report = as_text(TOOL, stdout)?.trim();
    let entries: Vec<serde_json::Value> = serde_json::from_str::<Option<Vec<_>>>(report)
        .map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?
        .unwrap_or_default();
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        let checked = known_spelled(TOOL, files, text(entry, "filepath")?)?;
        let message = text(entry, "message")?.trim();
        if message.is_empty() {
            return Err(ParseError::shape(TOOL, "missing message"));
        }
        let kind = text(entry, "kind")?;
        if kind.is_empty() {
            return Err(ParseError::shape(TOOL, "missing kind"));
        }
        let (start, end) = point(position(entry, "line")?, position(entry, "column")?);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: kind.to_owned(),
                message: message.to_owned(),
                severity: ToolSeverity::Error,
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

    const FILE: &str = ".github/workflows/ci.yml";
    const DIRTY: &str = r#"[{"message":"property \"x\" is not defined in object type {}","filepath":".github/workflows/ci.yml","line":9,"column":14,"kind":"expression","snippet":"","end_column":20},{"message":"shellcheck reported issue","filepath":".github/workflows/ci.yml","line":12,"column":9,"kind":"shellcheck","snippet":"","end_column":10}]"#;

    #[test]
    fn actionlint_clean_run_is_clean() {
        assert!(
            parse_actionlint(b"null", b"", Some(0), &[FILE])
                .expect("parsed")
                .is_empty()
        );
        assert!(
            parse_actionlint(b"[]\n", b"", Some(0), &[FILE])
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn actionlint_reports_kind_as_rule() {
        let findings = parse_actionlint(DIRTY.as_bytes(), b"", Some(1), &[FILE]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.rule_id, "expression");
        assert_eq!(findings[0].finding.start.line, 9);
        assert_eq!(findings[0].finding.start.column, 14);
        assert_eq!(findings[1].finding.rule_id, "shellcheck");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn actionlint_failures_are_errors() {
        assert!(parse_actionlint(b"", b"bad flag", Some(2), &[FILE]).is_err());
        assert!(parse_actionlint(b"", b"", Some(3), &[FILE]).is_err());
        assert!(parse_actionlint(b"null", b"", Some(1), &[FILE]).is_err());
        assert!(parse_actionlint(b"", b"", Some(0), &[FILE]).is_err());
        assert!(parse_actionlint(b"{}", b"", Some(1), &[FILE]).is_err());
        assert!(parse_actionlint(&[0xff], b"", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn actionlint_rejects_unknown_files() {
        let err = parse_actionlint(DIRTY.as_bytes(), b"", Some(1), &["other.yml"])
            .expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
