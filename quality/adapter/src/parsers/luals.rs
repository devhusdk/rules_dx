use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, finding, FileFinding, ParseError};

const TOOL: &str = "luals";

/// Findings from the `--check_out_path` report of `lua-language-server --check`.
///
/// The report is a JSON object that maps the `file://` URI of each file with diagnostics to a
/// list of LSP diagnostics; a clean run writes an empty array (an empty object is read the
/// same way). The exit code says nothing about findings (LuaLS exits 0 with them), so the
/// caller must treat a missing report as a failed run. `files` holds the paths of the owned
/// sources as the run spells them; an entry for any other file (a library or dependency
/// module) is dropped. The rule is `luals/<code>`, `luals/error` when LuaLS gave no code;
/// severity 1 is an error, 2 a warning and 3 or 4 (information, hint) info.
pub fn parse_luals(stdout: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let report: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|err| ParseError::shape(TOOL, format!("report is not JSON: {err}")))?;
    let by_uri = match &report {
        serde_json::Value::Array(empty) if empty.is_empty() => return Ok(Vec::new()),
        serde_json::Value::Object(map) => map,
        _ => {
            return Err(ParseError::shape(
                TOOL,
                "report is not an object".to_owned(),
            ))
        }
    };
    let mut findings: Vec<FileFinding> = Vec::new();
    for (uri, entries) in by_uri {
        let path = uri_path(uri)
            .ok_or_else(|| ParseError::shape(TOOL, format!("key {uri} is not a file URI")))?;
        if !files.contains(&path.as_str()) {
            continue;
        }
        let entries = entries
            .as_array()
            .ok_or_else(|| ParseError::shape(TOOL, format!("{path} diagnostics are not a list")))?;
        for entry in entries {
            findings.push(one(&path, entry)?);
        }
    }
    findings.sort_by(|a, b| {
        (&a.file, a.finding.start.line, a.finding.start.column).cmp(&(
            &b.file,
            b.finding.start.line,
            b.finding.start.column,
        ))
    });
    Ok(findings)
}

fn one(path: &str, entry: &serde_json::Value) -> Result<FileFinding, ParseError> {
    let message = entry
        .get("message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} message is not text")))?;
    let start = entry
        .get("range")
        .and_then(|range| range.get("start"))
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} diagnostic without a range")))?;
    let at = |key: &str| start.get(key).and_then(serde_json::Value::as_u64);
    let line = at("line")
        .ok_or_else(|| ParseError::shape(TOOL, format!("{path} range without a line")))?;
    let column = at("character").unwrap_or(0);
    // The code is a string, or a number in some LuaLS versions.
    let rule = match entry.get("code") {
        Some(serde_json::Value::String(code)) if !code.is_empty() => format!("luals/{code}"),
        Some(serde_json::Value::Number(code)) => format!("luals/{code}"),
        _ => "luals/error".to_owned(),
    };
    let severity = match entry.get("severity").and_then(serde_json::Value::as_u64) {
        Some(1) | None => ToolSeverity::Error,
        Some(2) => ToolSeverity::Warning,
        Some(_) => ToolSeverity::Info,
    };
    Ok(finding(
        TOOL,
        path,
        rule,
        message.to_owned(),
        severity,
        TextPosition {
            line: line.saturating_add(1),
            column: column.saturating_add(1),
        },
        ColumnEncoding::Utf16,
    ))
}

/// The file path a `file://` URI names: percent escapes decoded, and a Windows drive URI
/// (`file:///C:/x`) turned into `C:/x`.
fn uri_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    let path = String::from_utf8(out).ok()?;
    let drive = path.as_bytes();
    if drive.len() >= 3 && drive[0] == b'/' && drive[1].is_ascii_alphabetic() && drive[2] == b':' {
        return Some(path[1..].to_owned());
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"file:///s/a.lua":[{"code":"undefined-global","message":"Undefined global `x`.","range":{"start":{"line":5,"character":6},"end":{"line":5,"character":7}},"severity":2,"source":"Lua Diagnostics."},{"code":"param-type-mismatch","message":"Cannot assign `string` to parameter `integer`.","range":{"start":{"line":4,"character":12},"end":{"line":4,"character":15}},"severity":1}],"file:///s/dx-luals/mod/dep.lua":[{"code":"x","message":"dep","range":{"start":{"line":0,"character":0}},"severity":2}]}"#;

    #[test]
    fn luals_reports_owned_findings_sorted_with_rules_and_severities() {
        let found = parse_luals(REPORT.as_bytes(), &["/s/a.lua"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.file.as_str(),
                    f.finding.start.line,
                    f.finding.start.column,
                    f.finding.rule_id.as_str(),
                    f.finding.severity,
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                (
                    "/s/a.lua",
                    5,
                    13,
                    "luals/param-type-mismatch",
                    ToolSeverity::Error
                ),
                (
                    "/s/a.lua",
                    6,
                    7,
                    "luals/undefined-global",
                    ToolSeverity::Warning
                ),
            ]
        );
    }

    #[test]
    fn luals_clean_reports_have_no_findings() {
        assert!(parse_luals(b"[]", &["/s/a.lua"]).expect("array").is_empty());
        assert!(parse_luals(b"{}", &["/s/a.lua"])
            .expect("object")
            .is_empty());
        assert!(parse_luals(REPORT.as_bytes(), &["/s/other.lua"])
            .expect("library only")
            .is_empty());
    }

    #[test]
    fn luals_decodes_percent_escapes_and_windows_drives() {
        let report = r#"{"file:///s/my%20dir/a.lua":[{"message":"m","range":{"start":{"line":0,"character":0}},"severity":3,"code":7}],"file:///C:/w/b.lua":[{"message":"m","range":{"start":{"line":0,"character":0}},"severity":4}]}"#;
        let found =
            parse_luals(report.as_bytes(), &["/s/my dir/a.lua", "C:/w/b.lua"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| (f.file.as_str(), f.finding.rule_id.as_str()))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("/s/my dir/a.lua", "luals/7"),
                ("C:/w/b.lua", "luals/error")
            ]
        );
        assert!(found
            .iter()
            .all(|f| f.finding.severity == ToolSeverity::Info));
    }

    #[test]
    fn luals_malformed_reports_fail_the_run() {
        assert!(parse_luals(b"", &[]).is_err());
        assert!(parse_luals(b"not json", &[]).is_err());
        assert!(parse_luals(b"\"x\"", &[]).is_err());
        assert!(parse_luals(br#"{"/s/a.lua":[]}"#, &["/s/a.lua"]).is_err());
        assert!(parse_luals(br#"{"file:///s/a.lua":{}}"#, &["/s/a.lua"]).is_err());
        assert!(parse_luals(br#"{"file:///s/a.lua":[{"message":"m"}]}"#, &["/s/a.lua"]).is_err());
    }
}
