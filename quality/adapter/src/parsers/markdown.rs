use serde::Deserialize;

use super::{check_output_size, code_name, known, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

#[derive(Debug, Deserialize)]
struct MarkdownLine {
    path: String,
    line: u64,
    kind: String,
    message: String,
}

pub fn parse_markdown_findings(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "markdown_check";
    check_output_size(TOOL, stdout)?;
    const KINDS: &[&str] = &[
        "missing-file-target",
        "missing-anchor",
        "heading-hierarchy",
        "missing-code-fence-language",
        "unclosed-code-fence",
    ];
    if code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {}: findings exist only on exit 0", code_name(code)),
        });
    }
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: MarkdownLine = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        if !KINDS.contains(&parsed.kind.as_str()) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unknown finding kind {:?}", parsed.kind),
            });
        }
        if parsed.line == 0 {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("zero line for {}", parsed.kind),
            });
        }
        let checked = known(TOOL, files, &parsed.path)?;
        let (start, end) = point(parsed.line, 1);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: parsed.kind,
                message: parsed.message,
                severity: ToolSeverity::Error,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    #[test]
    fn markdown_parses_workspace_keyed_findings() {
        let stdout = concat!(
            "{\"path\":\"doc/guide.md\",\"line\":3,\"kind\":\"missing-file-target\",\"message\":\"link target \\\"nope.md\\\" does not match a checked source\"}\n",
            "{\"path\":\"doc/guide.md\",\"line\":7,\"kind\":\"missing-anchor\",\"message\":\"anchor \\\"#nope\\\" not found\"}\n",
        );
        let findings =
            parse_markdown_findings(stdout.as_bytes(), Some(0), &["doc/guide.md"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "doc/guide.md");
        assert_eq!(findings[0].finding.tool_id, "markdown_check");
        assert_eq!(findings[0].finding.rule_id, "missing-file-target");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 3, column: 1 }, None)
        );
        assert_eq!(findings[1].finding.rule_id, "missing-anchor");
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_markdown_findings(b"\n", Some(0), &["doc/guide.md"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn markdown_accepts_every_current_checker_kind() {
        let stdout = concat!(
            "{\"path\":\"doc/guide.md\",\"line\":3,\"kind\":\"missing-file-target\",\"message\":\"t\"}\n",
            "{\"path\":\"doc/guide.md\",\"line\":4,\"kind\":\"missing-anchor\",\"message\":\"a\"}\n",
            "{\"path\":\"doc/guide.md\",\"line\":1,\"kind\":\"heading-hierarchy\",\"message\":\"h\"}\n",
            "{\"path\":\"doc/guide.md\",\"line\":5,\"kind\":\"missing-code-fence-language\",\"message\":\"l\"}\n",
            "{\"path\":\"doc/guide.md\",\"line\":5,\"kind\":\"unclosed-code-fence\",\"message\":\"u\"}\n",
        );
        let findings =
            parse_markdown_findings(stdout.as_bytes(), Some(0), &["doc/guide.md"]).expect("parsed");
        assert_eq!(findings.len(), 5);
        assert_eq!(findings[2].finding.rule_id, "heading-hierarchy");
        let stale =
            "{\"path\":\"doc/guide.md\",\"line\":1,\"kind\":\"missing-heading\",\"message\":\"m\"}";
        assert!(parse_markdown_findings(stale.as_bytes(), Some(0), &["doc/guide.md"]).is_err());
    }

    #[test]
    fn markdown_rejects_exits_kinds_lines_and_files() {
        let clean =
            "{\"path\":\"doc/guide.md\",\"line\":1,\"kind\":\"heading-hierarchy\",\"message\":\"m\"}";
        assert!(parse_markdown_findings(clean.as_bytes(), Some(1), &["doc/guide.md"]).is_err());
        assert!(parse_markdown_findings(clean.as_bytes(), None, &["doc/guide.md"]).is_err());
        assert!(parse_markdown_findings(b"", Some(2), &["doc/guide.md"]).is_err());
        assert!(parse_markdown_findings(b"\xff", Some(0), &["doc/guide.md"]).is_err());
        assert!(parse_markdown_findings(b"{nope", Some(0), &["doc/guide.md"]).is_err());
        let missing_field = r#"{"path":"doc/guide.md","line":1,"kind":"missing-heading"}"#;
        assert!(
            parse_markdown_findings(missing_field.as_bytes(), Some(0), &["doc/guide.md"]).is_err()
        );
        let unknown_kind = r#"{"path":"doc/guide.md","line":1,"kind":"bad-kind","message":"m"}"#;
        assert!(
            parse_markdown_findings(unknown_kind.as_bytes(), Some(0), &["doc/guide.md"]).is_err()
        );
        let zero_line =
            r#"{"path":"doc/guide.md","line":0,"kind":"heading-hierarchy","message":"m"}"#;
        assert!(parse_markdown_findings(zero_line.as_bytes(), Some(0), &["doc/guide.md"]).is_err());
        let elsewhere = r#"{"path":"other.md","line":1,"kind":"heading-hierarchy","message":"m"}"#;
        assert!(parse_markdown_findings(elsewhere.as_bytes(), Some(0), &["doc/guide.md"]).is_err());
    }
}
