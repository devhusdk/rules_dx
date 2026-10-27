use super::{check_output_size, code_name, located, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};

fn flake8_severity(code: &str) -> Result<ToolSeverity, ParseError> {
    const TOOL: &str = "flake8";
    match code.chars().next() {
        Some('E') | Some('F') => Ok(ToolSeverity::Error),
        Some('W') | Some('C') => Ok(ToolSeverity::Warning),
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unknown code family: {code:?}"),
        }),
    }
}

fn flake8_line<'a>(
    line: &'a str,
    files: &[&'a str],
) -> Result<(&'a str, u64, u64, &'a str, &'a str), ParseError> {
    const TOOL: &str = "flake8";
    let (path, rest) = located(TOOL, line, files)?;
    let mut parts = rest.splitn(4, ':');
    let (row_text, column_text, code, message) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    let (row, column) = (row_text.parse::<u64>(), column_text.parse::<u64>());
    match (
        path.is_empty(),
        row,
        column,
        code.is_empty(),
        message.is_empty(),
    ) {
        (false, Ok(row), Ok(column), false, false) if row >= 1 && column >= 1 => {
            Ok((path, row, column, code, message))
        }
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed finding: {line:?}"),
        }),
    }
}

pub fn parse_flake8(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "flake8";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (path, row, column, rule, message) = flake8_line(line, files)?;
        let (start, end) = point(row, column);
        findings.push(FileFinding {
            file: path.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule.to_owned(),
                message: message.to_owned(),
                severity: flake8_severity(rule)?,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no findings", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const FLAKE8_DIRTY: &str = concat!(
        "/s/dirty.py:3:1:F401:'os' imported but unused\n",
        "/s/dirty.py:22:6:E231:missing whitespace after ':'\n",
        "/s/dirty.py:22:10:E225:missing whitespace around operator\n",
    );

    #[test]
    fn flake8_reports_points_with_family_severity() {
        let findings =
            parse_flake8(FLAKE8_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].file, "/s/dirty.py");
        assert_eq!(findings[0].finding.tool_id, "flake8");
        assert_eq!(findings[0].finding.rule_id, "F401");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 3, column: 1 }, None)
        );
        assert_eq!(findings[0].finding.message, "'os' imported but unused");
        assert_eq!(findings[1].finding.rule_id, "E231");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[2].finding.rule_id, "E225");
        assert!(findings
            .iter()
            .all(|finding| finding.finding.suggestions.is_empty()));
        assert!(parse_flake8(b"", Some(0), &["/s/dirty.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_flake8(b"", Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_flake8(FLAKE8_DIRTY.as_bytes(), Some(1), &["/s/other.py"]).is_err());
        assert!(parse_flake8(b"/s/a.py:1:1:X999:made up\n", Some(1), &["/s/a.py"]).is_err());
        assert!(parse_flake8(b"garbage\n", Some(1), &["/s/a.py"]).is_err());
    }

    #[test]
    fn flake8_survives_colons_inside_the_message() {
        let stdout = "/s/broken.py:1:1:E999:SyntaxError: invalid syntax\n";
        let findings = parse_flake8(stdout.as_bytes(), Some(1), &["/s/broken.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "E999");
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 1, column: 1 }, None)
        );
        assert_eq!(findings[0].finding.message, "SyntaxError: invalid syntax");
        let stdout = "/s/a.py:1:80:W505:doc line too long: fix it\n/s/a.py:2:1:C901:function is too complex\n";
        let findings = parse_flake8(stdout.as_bytes(), Some(1), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .all(|finding| finding.finding.severity == ToolSeverity::Warning));
    }

    #[test]
    fn flake8_keeps_the_drive_of_a_windows_path() {
        let stdout = "C:\\s\\a.py:1:1:F401:'os' imported but unused\n";
        let findings = parse_flake8(stdout.as_bytes(), Some(1), &["C:\\s\\a.py"]).expect("parsed");
        assert_eq!(findings[0].file, "C:\\s\\a.py");
        assert_eq!(findings[0].finding.rule_id, "F401");
    }

    #[test]
    fn flake8_grammar_mismatches_are_fail_closed() {
        assert!(parse_flake8(&[0xff], Some(1), &["/s/dirty.py"]).is_err());
        let blanked_flake8 = "\n/s/dirty.py:3:1:F401:msg\n\n";
        let findings =
            parse_flake8(blanked_flake8.as_bytes(), Some(1), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn flake8_reads_drive_rooted_paths() {
        let file = "C:\\tmp\\a.py";
        let findings = parse_flake8(
            "C:\\tmp\\a.py:3:1:E501:line too long\n".as_bytes(),
            Some(1),
            &[file],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, file);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 1);
    }
}
