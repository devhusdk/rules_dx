use super::{FileFinding, ParseError, as_text, code_name, known_spelled, point, require_findings};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "lychee";

/// Parses `lychee --format json`: stdout is one report, exit 2 means some link failed.
///
/// Failed links sit under `error_map` (`fail_map` before 0.20) and `timeout_map`, keyed by
/// the file lychee was given.
pub fn parse_lychee(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 2)) {
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
        serde_json::from_str(as_text(TOOL, stdout)?).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::new();
    for key in ["error_map", "fail_map", "timeout_map"] {
        let Some(map) = report.get(key) else {
            continue;
        };
        let map = map
            .as_object()
            .ok_or_else(|| ParseError::shape(TOOL, format!("{key} is not an object")))?;
        for (file, links) in map {
            let checked = known_spelled(TOOL, files, file)?;
            let links = links
                .as_array()
                .ok_or_else(|| ParseError::shape(TOOL, format!("{key} entry is not a list")))?;
            for link in links {
                findings.push(link_finding(checked, link)?);
            }
        }
    }
    if report.get("error_map").is_none() && report.get("fail_map").is_none() {
        return Err(ParseError::shape(TOOL, "missing error_map"));
    }
    findings.sort_by(|left, right| {
        (
            left.file.as_str(),
            left.finding.start.line,
            left.finding.start.column,
        )
            .cmp(&(
                right.file.as_str(),
                right.finding.start.line,
                right.finding.start.column,
            ))
    });
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

fn link_finding(file: &str, link: &serde_json::Value) -> Result<FileFinding, ParseError> {
    let url = link
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, "missing url"))?;
    let status = link
        .get("status")
        .and_then(|status| status.get("text"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ParseError::shape(TOOL, "missing status text"))?;
    let span = |key: &str| {
        link.get("span")
            .and_then(|span| span.get(key))
            .and_then(serde_json::Value::as_u64)
            .map_or(1, |number| number.max(1))
    };
    let (start, end) = point(span("line"), span("column"));
    Ok(FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: "broken-link".to_owned(),
            message: format!("{url}: {status}"),
            severity: ToolSeverity::Error,
            start,
            end,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"total":3,"errors":2,"success_map":{},"error_map":{"/s/README.md":[{"url":"file:///s/docs/missing.md","status":{"text":"File not found. Check if file exists and path is correct","details":"x"},"span":{"line":4,"column":1}},{"url":"file:///s/docs/x.md#nope","status":{"text":"Cannot find fragment","details":"x"},"span":{"line":3,"column":7}}]},"timeout_map":{},"excluded_map":{"/s/README.md":[{"url":"https://example.com/","status":{"text":"Excluded"},"span":{"line":5,"column":1}}]}}"#;
    const CLEAN: &str =
        r#"{"total":1,"errors":0,"success_map":{},"error_map":{},"timeout_map":{}}"#;

    #[test]
    fn lychee_clean_run_is_clean() {
        let found =
            parse_lychee(CLEAN.as_bytes(), b"", Some(0), &["/s/README.md"]).expect("parsed");
        assert!(found.is_empty());
    }

    #[test]
    fn lychee_reports_failed_links_in_line_order() {
        let found =
            parse_lychee(DIRTY.as_bytes(), b"", Some(2), &["/s/README.md"]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].file, "/s/README.md");
        assert_eq!(found[0].finding.tool_id, "lychee");
        assert_eq!(found[0].finding.rule_id, "broken-link");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 7);
        assert_eq!(
            found[0].finding.message,
            "file:///s/docs/x.md#nope: Cannot find fragment"
        );
        assert_eq!(found[1].finding.start.line, 4);
    }

    #[test]
    fn lychee_reads_the_older_fail_map() {
        let old = DIRTY.replace("error_map", "fail_map");
        let found = parse_lychee(old.as_bytes(), b"", Some(2), &["/s/README.md"]).expect("parsed");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn lychee_runtime_errors_are_not_findings() {
        let err = parse_lychee(b"", b"Error: Cannot parse inputs\n", Some(1), &["/s/a.md"])
            .expect_err("exit 1");
        assert!(err.to_string().contains("Cannot parse inputs"));
        assert!(parse_lychee(b"not json", b"", Some(0), &["/s/a.md"]).is_err());
        assert!(parse_lychee(b"{}", b"", Some(0), &["/s/a.md"]).is_err());
    }

    #[test]
    fn lychee_exit_two_without_findings_and_unknown_files_are_errors() {
        assert!(parse_lychee(CLEAN.as_bytes(), b"", Some(2), &["/s/README.md"]).is_err());
        assert!(parse_lychee(DIRTY.as_bytes(), b"", Some(2), &["/s/other.md"]).is_err());
    }
}
