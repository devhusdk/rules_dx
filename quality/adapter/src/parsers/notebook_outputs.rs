use serde::Deserialize;

use super::{check_output_size, code_name, known_native, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputLine {
    path: String,
    cell: u64,
    line: u64,
    column: u64,
    rule: String,
    message: String,
}

/// Parses `quality_notebook outputs`: JSON lines on stdout, exit 0 when
/// clean and exit 1 when there are findings. Anything else fails closed.
pub fn parse_notebook_outputs(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "notebook_outputs";
    const RULES: &[&str] = &["cell-outputs", "execution-count"];
    check_output_size(TOOL, stdout)?;
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {}: expected 0 or 1", code_name(code)),
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
        let parsed: OutputLine = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        if !RULES.contains(&parsed.rule.as_str()) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unknown rule {:?}", parsed.rule),
            });
        }
        if parsed.line == 0 || parsed.column == 0 || parsed.cell == 0 {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("zero position for {}", parsed.rule),
            });
        }
        let checked = known_native(TOOL, files, &parsed.path)?;
        let (start, end) = point(parsed.line, parsed.column);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: parsed.rule,
                message: parsed.message,
                severity: ToolSeverity::Error,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if code == Some(0) && !findings.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "findings on exit 0".to_owned(),
        });
    }
    if code == Some(1) && findings.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "exit 1 with no findings".to_owned(),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const DIRTY: &str = "{\"path\":\"/w/a.ipynb\",\"cell\":2,\"line\":9,\"column\":4,\"rule\":\"cell-outputs\",\"message\":\"cell 2: code cell has saved outputs\"}\n";

    #[test]
    fn clean_exit_zero_has_no_findings() {
        assert!(parse_notebook_outputs(b"", Some(0), &["/w/a.ipynb"])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn dirty_exit_one_parses_findings() {
        let found = parse_notebook_outputs(DIRTY.as_bytes(), Some(1), &["/w/a.ipynb"]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/w/a.ipynb");
        assert_eq!(found[0].finding.rule_id, "cell-outputs");
        assert_eq!(found[0].finding.start, TextPosition { line: 9, column: 4 });
        assert!(found[0].finding.message.starts_with("cell 2:"));
    }

    #[test]
    fn fails_closed() {
        let files = ["/w/a.ipynb"];
        assert!(parse_notebook_outputs(DIRTY.as_bytes(), Some(0), &files).is_err());
        assert!(parse_notebook_outputs(b"", Some(1), &files).is_err());
        assert!(parse_notebook_outputs(b"not json\n", Some(1), &files).is_err());
        assert!(parse_notebook_outputs(DIRTY.as_bytes(), Some(2), &files).is_err());
        assert!(parse_notebook_outputs(b"", None, &files).is_err());
        assert!(parse_notebook_outputs(DIRTY.as_bytes(), Some(1), &["/w/b.ipynb"]).is_err());
        let bad = DIRTY.replace("cell-outputs", "other");
        assert!(parse_notebook_outputs(bad.as_bytes(), Some(1), &files).is_err());
    }
}
