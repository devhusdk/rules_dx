use super::{as_text, code_name, known_spelled, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "knip";

fn finding(file: &str, rule_id: &str, message: String, start: TextPosition) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule_id.to_owned(),
            message,
            severity: ToolSeverity::Warning,
            start,
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

fn array<'a>(value: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Knip omits the position of some entries; those land on the first character.
fn position(entry: &serde_json::Value) -> TextPosition {
    let axis = |key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .filter(|value| *value > 0)
            .unwrap_or(1)
    };
    TextPosition {
        line: axis("line"),
        column: axis("col"),
    }
}

fn name(entry: &serde_json::Value) -> Result<&str, ParseError> {
    entry
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, "entry without a name"))
}

/// Parses `knip --reporter json`: unused files, unused exports and unused dependencies.
/// Paths are relative to the package root the run started in. Exit 0 is clean and 1 means
/// issues; anything else is a failure.
pub fn parse_knip(
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
    let report: serde_json::Value =
        serde_json::from_str(as_text(TOOL, stdout)?.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON object ({err})"),
        })?;
    if !report.is_object() {
        return Err(ParseError::shape(TOOL, "stdout must be one JSON object"));
    }
    let mut findings = Vec::new();
    for path in array(&report, "files") {
        let path = path
            .as_str()
            .ok_or_else(|| ParseError::shape(TOOL, "file entry is not a string"))?;
        findings.push(finding(
            known_spelled(TOOL, files, path)?,
            "files",
            "Unused file".to_owned(),
            TextPosition { line: 1, column: 1 },
        ));
    }
    for issue in array(&report, "issues") {
        let path = issue
            .get("file")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ParseError::shape(TOOL, "issue without a file"))?;
        for (key, rule_id, what) in [
            ("exports", "exports", "Unused export"),
            ("dependencies", "dependencies", "Unused dependency"),
        ] {
            for entry in array(issue, key) {
                findings.push(finding(
                    known_spelled(TOOL, files, path)?,
                    rule_id,
                    format!("{what} `{}`", name(entry)?),
                    position(entry),
                ));
            }
        }
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"files":["src/dead.ts"],"issues":[
{"file":"package.json","dependencies":[{"name":"left-pad","line":6,"col":6,"pos":120}],"exports":[]},
{"file":"src/a.ts","dependencies":[],"exports":[{"name":"unused","line":3,"col":14,"pos":40}],"types":[{"name":"T","line":1,"col":1}]}
]}"#;
    const FILES: [&str; 3] = ["package.json", "src/a.ts", "src/dead.ts"];

    #[test]
    fn knip_clean_run_is_clean() {
        let clean = br#"{"files":[],"issues":[]}"#;
        assert!(parse_knip(clean, b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn knip_reports_files_exports_and_dependencies_as_info() {
        let findings = parse_knip(DIRTY.as_bytes(), b"", Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].file, "src/dead.ts");
        assert_eq!(findings[0].finding.rule_id, "files");
        assert_eq!(findings[1].file, "package.json");
        assert_eq!(findings[1].finding.rule_id, "dependencies");
        assert_eq!(findings[1].finding.message, "Unused dependency `left-pad`");
        assert_eq!(findings[2].file, "src/a.ts");
        assert_eq!(findings[2].finding.rule_id, "exports");
        assert_eq!(findings[2].finding.start.line, 3);
        assert_eq!(findings[2].finding.start.column, 14);
        assert!(findings
            .iter()
            .all(|found| found.finding.severity == ToolSeverity::Warning));
    }

    #[test]
    fn knip_entries_without_a_position_start_at_one_one() {
        let report = br#"{"files":[],"issues":[{"file":"src/a.ts","exports":[{"name":"x"}]}]}"#;
        let findings = parse_knip(report, b"", Some(1), &FILES).expect("parsed");
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 1, column: 1 }
        );
    }

    #[test]
    fn knip_failures_are_errors() {
        assert!(parse_knip(b"", b"boom", Some(2), &FILES).is_err());
        assert!(parse_knip(b"{}", b"", None, &FILES).is_err());
        assert!(parse_knip(b"", b"", Some(0), &FILES).is_err());
        assert!(parse_knip(b"[]", b"", Some(0), &FILES).is_err());
        assert!(parse_knip(br#"{"files":[],"issues":[]}"#, b"", Some(1), &FILES).is_err());
        assert!(parse_knip(br#"{"files":[1]}"#, b"", Some(1), &FILES).is_err());
    }

    #[test]
    fn knip_rejects_unknown_files() {
        let err = parse_knip(DIRTY.as_bytes(), b"", Some(1), &["other.ts"]).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
