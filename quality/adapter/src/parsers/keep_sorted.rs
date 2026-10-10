use super::{
    check_output_size, code_name, finding, known, lines, located, missing, numbered,
    require_findings, FileFinding, ParseError,
};
use crate::{Finding, TextPosition, ToolSeverity};

fn json_fail(tool: &'static str, detail: String) -> ParseError {
    ParseError::Json { tool, detail }
}

fn shape_fail(tool: &'static str, detail: String) -> ParseError {
    ParseError::Shape { tool, detail }
}

pub fn parse_keep_sorted(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "keep_sorted";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let mut parts = rest.splitn(2, ':');
        let line_no = numbered(TOOL, "line", line, parts.next())?;
        let message = parts
            .next()
            .ok_or_else(|| missing(TOOL, "message", line))?
            .trim();
        if message.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.to_owned(),
            ToolSeverity::Warning,
            line_no,
            1,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

fn ranged(
    value: &serde_json::Value,
    what: &str,
) -> Result<(TextPosition, TextPosition), ParseError> {
    const TOOL: &str = "keep_sorted";
    let lines = value
        .get("lines")
        .ok_or_else(|| shape_fail(TOOL, format!("finding {what} lacks lines")))?;
    let start = lines
        .get("start")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| shape_fail(TOOL, format!("finding {what} lacks a start line")))?;
    let end = lines
        .get("end")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| shape_fail(TOOL, format!("finding {what} lacks an end line")))?;
    if start == 0 || end == 0 || end < start {
        return Err(shape_fail(
            TOOL,
            format!("finding {what} carries a bad line range"),
        ));
    }
    Ok((
        TextPosition {
            line: start,
            column: 1,
        },
        TextPosition {
            line: end,
            column: 1,
        },
    ))
}

pub fn parse_keep_sorted_direct(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "keep_sorted";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| shape_fail(TOOL, err.to_string()))?;
    if text.trim().is_empty() {
        require_findings(TOOL, &[], code)?;
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| json_fail(TOOL, err.to_string()))?;
    let items = value
        .as_array()
        .ok_or_else(|| shape_fail(TOOL, "top level is not an array".to_owned()))?;
    let mut findings = Vec::with_capacity(items.len());
    for item in items {
        let path = item
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a path".to_owned()))?;
        let checked = known(TOOL, files, path)?;
        let (start, end) = ranged(item, "finding")?;
        let message = item
            .get("message")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a message".to_owned()))?;
        if message.is_empty() {
            return Err(shape_fail(
                TOOL,
                "finding carries an empty message".to_owned(),
            ));
        }
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: String::new(),
                message: message.to_owned(),
                severity: ToolSeverity::Warning,
                start,
                end: Some(end),
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(shape_fail(
            TOOL,
            format!("exit {} with no findings", code_name(code)),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "notes.txt:4: block is not sorted\n";
    #[test]
    fn keep_sorted_reports_line_points() {
        let findings =
            parse_keep_sorted(DIRTY.as_bytes(), Some(1), &["notes.txt"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_keep_sorted(b"", Some(0), &["notes.txt"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_keep_sorted(b"", Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted(DIRTY.as_bytes(), Some(1), &["other.txt"]).is_err());
        assert!(parse_keep_sorted(&[0xff], Some(1), &["x"]).is_err());
    }

    const DIRECT_SINGLE: &str = r#"[
  {
    "path": "notes.txt",
    "lines": {
      "start": 2,
      "end": 4
    },
    "message": "These lines are out of order.",
    "fixes": [
      {
        "replacements": [
          {
            "lines": {
              "start": 2,
              "end": 4
            },
            "new_content": "a\nb\nc\n"
          }
        ]
      }
    ]
  }
]"#;

    const DIRECT_MULTI: &str = r#"[
  {
    "path": "notes.txt",
    "lines": {"start": 2, "end": 4},
    "message": "These lines are out of order.",
    "fixes": []
  },
  {
    "path": "notes.txt",
    "lines": {"start": 8, "end": 9},
    "message": "These lines are out of order.",
    "fixes": []
  }
]"#;

    #[test]
    fn keep_sorted_direct_reports_json_ranges() {
        let findings = parse_keep_sorted_direct(DIRECT_SINGLE.as_bytes(), Some(1), &["notes.txt"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "notes.txt");
        assert_eq!(findings[0].finding.message, "These lines are out of order.");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(
            findings[0].finding.end,
            Some(TextPosition { line: 4, column: 1 })
        );
        let multi = parse_keep_sorted_direct(DIRECT_MULTI.as_bytes(), Some(1), &["notes.txt"])
            .expect("parsed");
        assert_eq!(multi.len(), 2);
        assert_eq!(multi[1].finding.start.line, 8);
        let clean = parse_keep_sorted_direct(b"", Some(0), &["notes.txt"]).expect("parsed");
        assert!(clean.is_empty());
        let empty_array = parse_keep_sorted_direct(b"[]", Some(0), &["notes.txt"]).expect("parsed");
        assert!(empty_array.is_empty());
        assert!(parse_keep_sorted_direct(b"", Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted_direct(b"[]", Some(1), &["notes.txt"]).is_err());
        assert!(
            parse_keep_sorted_direct(DIRECT_SINGLE.as_bytes(), Some(1), &["other.txt"]).is_err()
        );
        assert!(parse_keep_sorted_direct(b"{}", Some(1), &["x"]).is_err());
        assert!(parse_keep_sorted_direct(DIRTY.as_bytes(), Some(1), &["notes.txt"]).is_err());
        assert!(parse_keep_sorted_direct(&[0xff], Some(1), &["x"]).is_err());
    }
}
