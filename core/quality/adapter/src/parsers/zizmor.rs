use super::{FileFinding, ParseError, as_text, code_name, known_spelled, require_findings};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "zizmor";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

fn severity(name: &str) -> Result<ToolSeverity, ParseError> {
    match name {
        "High" => Ok(ToolSeverity::Error),
        "Medium" | "Low" => Ok(ToolSeverity::Warning),
        "Informational" => Ok(ToolSeverity::Info),
        other => Err(ParseError::shape(
            TOOL,
            format!("unknown severity {other:?}"),
        )),
    }
}

/// The workflow path and the one-based start of the primary location of a finding.
fn primary(entry: &serde_json::Value) -> Result<(&str, TextPosition), ParseError> {
    let locations = entry
        .get("locations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ParseError::shape(TOOL, "missing locations"))?;
    let location = locations
        .iter()
        .find(|location| {
            location
                .pointer("/symbolic/kind")
                .and_then(serde_json::Value::as_str)
                == Some("Primary")
        })
        .or_else(|| locations.first())
        .ok_or_else(|| ParseError::shape(TOOL, "empty locations"))?;
    let path = location
        .pointer("/symbolic/key/Local/given_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, "missing given_path"))?;
    let start = |axis: &str| {
        location
            .pointer(&format!("/concrete/location/start_point/{axis}"))
            .and_then(serde_json::Value::as_u64)
            .map(|zero_based| zero_based + 1)
            .ok_or_else(|| ParseError::shape(TOOL, format!("missing start {axis}")))
    };
    Ok((
        path,
        TextPosition {
            line: start("row")?,
            column: start("column")?,
        },
    ))
}

/// Parses `zizmor --format json`: one JSON array. Exit 0 is clean and 10 to 14 mean findings
/// at the given highest severity; anything else is a failure.
pub fn parse_zizmor(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 10..=14)) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {}: {}",
                code_name(code),
                String::from_utf8_lossy(stderr).trim()
            ),
        ));
    }
    let entries: Vec<serde_json::Value> = serde_json::from_str(as_text(TOOL, stdout)?.trim())
        .map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut findings = Vec::with_capacity(entries.len());
    for entry in &entries {
        if entry.get("ignored").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let rule_id = text(entry, "ident")?;
        let message = text(entry, "desc")?.trim();
        if rule_id.is_empty() || message.is_empty() {
            return Err(ParseError::shape(TOOL, "missing ident or desc"));
        }
        let level = entry
            .pointer("/determinations/severity")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ParseError::shape(TOOL, "missing severity"))?;
        let (path, start) = primary(entry)?;
        findings.push(FileFinding {
            file: known_spelled(TOOL, files, path)?.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule_id.to_owned(),
                message: message.to_owned(),
                severity: severity(level)?,
                start,
                end: None,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Utf8,
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
    const DIRTY: &str = r#"[
{"ident":"unpinned-uses","desc":"unpinned action reference","url":"https://docs.zizmor.sh/audits/#unpinned-uses","determinations":{"confidence":"High","severity":"Medium"},"locations":[{"symbolic":{"key":{"Local":{"prefix":"/w","given_path":".github/workflows/ci.yml"}},"annotation":"action is not pinned","kind":"Primary"},"concrete":{"location":{"start_point":{"row":8,"column":14},"end_point":{"row":8,"column":35}},"feature":"actions/checkout@v4","comments":[]}}],"ignored":false},
{"ident":"template-injection","desc":"code injection via template expansion","url":"u","determinations":{"confidence":"High","severity":"High"},"locations":[{"symbolic":{"key":{"Local":{"prefix":"/w","given_path":".github/workflows/ci.yml"}},"annotation":"may expand into attacker-controllable code","kind":"Primary"},"concrete":{"location":{"start_point":{"row":11,"column":8},"end_point":{"row":11,"column":40}},"feature":"x","comments":[]}}],"ignored":false}
]"#;

    #[test]
    fn zizmor_clean_run_is_clean() {
        assert!(
            parse_zizmor(b"[]", b"", Some(0), &[FILE])
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn zizmor_reports_ident_severity_and_one_based_position() {
        let findings = parse_zizmor(DIRTY.as_bytes(), b"", Some(13), &[FILE]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.rule_id, "unpinned-uses");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.line, 9);
        assert_eq!(findings[0].finding.start.column, 15);
        assert_eq!(findings[1].finding.rule_id, "template-injection");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn zizmor_columns_are_utf8_bytes() {
        let dirty = DIRTY.replace("\"column\":8", "\"column\":29");
        let found = parse_zizmor(dirty.as_bytes(), b"", Some(13), &[FILE]).expect("parsed");
        assert_eq!(found[1].finding.start.column, 30);
        assert_eq!(found[1].finding.encoding, ColumnEncoding::Utf8);
    }

    #[test]
    fn zizmor_skips_ignored_findings() {
        let ignored = DIRTY.replace("\"ignored\":false", "\"ignored\":true");
        let findings = parse_zizmor(ignored.as_bytes(), b"", Some(0), &[FILE]).expect("parsed");
        assert!(findings.is_empty());
    }

    #[test]
    fn zizmor_failures_are_errors() {
        assert!(parse_zizmor(b"", b"boom", Some(1), &[FILE]).is_err());
        assert!(parse_zizmor(b"[]", b"", Some(2), &[FILE]).is_err());
        assert!(parse_zizmor(b"[]", b"", Some(11), &[FILE]).is_err());
        assert!(parse_zizmor(b"", b"", Some(0), &[FILE]).is_err());
        assert!(parse_zizmor(b"{}", b"", Some(0), &[FILE]).is_err());
        assert!(parse_zizmor(DIRTY.as_bytes(), b"", None, &[FILE]).is_err());
    }

    #[test]
    fn zizmor_rejects_unknown_files() {
        let err = parse_zizmor(DIRTY.as_bytes(), b"", Some(13), &["other.yml"])
            .expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
