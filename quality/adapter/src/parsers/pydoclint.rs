use super::{check_output_size, code_name, known_spelled, missing, point, FileFinding, ParseError};
use crate::{Finding, ToolSeverity};

fn pydoclint_violation(line: &str) -> Result<(u64, &str, String), ParseError> {
    const TOOL: &str = "pydoclint";
    let (line_text, rest) = line
        .split_once(':')
        .ok_or_else(|| missing(TOOL, "violation", line))?;
    let number = line_text.parse::<u64>().map_err(|_| ParseError::Shape {
        tool: TOOL,
        detail: format!("malformed violation: {line:?}"),
    })?;
    let rest = rest
        .strip_prefix(' ')
        .ok_or_else(|| missing(TOOL, "violation", line))?;
    let (rule, message) = rest
        .split_once(':')
        .ok_or_else(|| missing(TOOL, "violation", line))?;
    let message = message.strip_prefix(' ').unwrap_or(message);
    if !(rule.len() > 3
        && rule.starts_with("DOC")
        && rule[3..].chars().all(|char| char.is_ascii_digit()))
        || message.is_empty()
    {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed violation: {line:?}"),
        });
    }
    Ok((number, rule, message.to_owned()))
}

pub fn parse_pydoclint(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "pydoclint";
    check_output_size(TOOL, stderr)?;
    let text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    let mut current: Option<&str> = None;
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            let (number, rule, message) = pydoclint_violation(raw.trim())?;
            let header = current.ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: format!("violation without a file header: {raw:?}"),
            })?;
            let checked = known_spelled(TOOL, files, header)?;
            let (start, end) = point(number.max(1), 1);
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: rule.to_owned(),
                    message,
                    severity: ToolSeverity::Error,
                    start,
                    end,
                    suggestions: Vec::new(),
                },
            });
        } else {
            let header = raw.trim_end();
            known_spelled(TOOL, files, header)?;
            current = Some(header);
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no violations", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    #[test]
    fn pydoclint_reports_violations_per_header() {
        let stderr = concat!(
            "/s/a.py\n",
            "    4: DOC101: Function `foo`: Docstring contains fewer arguments than in function signature.\n",
            "    4: DOC201: Function `foo` does not have a return section in docstring\n",
        );
        let findings = parse_pydoclint(stderr.as_bytes(), Some(1), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.py");
        assert_eq!(findings[0].finding.tool_id, "pydoclint");
        assert_eq!(findings[0].finding.rule_id, "DOC101");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 4, column: 1 }, None)
        );
        assert_eq!(findings[1].finding.rule_id, "DOC201");
        assert!(parse_pydoclint(b"", Some(0), &["/s/a.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_pydoclint(b"", Some(1), &["/s/a.py"]).is_err());
        assert!(parse_pydoclint(stderr.as_bytes(), Some(1), &["/s/other.py"]).is_err());
        assert!(parse_pydoclint(b"    4: DOC101: msg\n", Some(1), &["/s/a.py"]).is_err());
    }

    #[test]
    fn pydoclint_syntax_error_points_at_file_top() {
        let stderr = "/s/broken.py\n    0: DOC002: Syntax errors; cannot parse this Python file.\n";
        let findings =
            parse_pydoclint(stderr.as_bytes(), Some(1), &["/s/broken.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "DOC002");
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 1, column: 1 }, None)
        );
    }

    /// pydoclint spells every path with `/`, so Windows needs both sides compared that way.
    #[test]
    fn pydoclint_reports_windows_paths_spelled_with_forward_slashes() {
        let stderr = "C:/tmp/dx-scratch-a/a.py\n    4: DOC101: msg\n";
        let checked = r"C:\tmp\dx-scratch-a\a.py";
        let findings = parse_pydoclint(stderr.as_bytes(), Some(1), &[checked]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, checked);
        assert!(
            parse_pydoclint(stderr.as_bytes(), Some(1), &[r"C:\tmp\dx-scratch-a\b.py"]).is_err()
        );
        assert!(parse_pydoclint(
            stderr.as_bytes(),
            Some(1),
            &[r"C:\tmp\dx-scratch-a\..\a.py"]
        )
        .is_err());
    }

    #[test]
    fn pydoclint_grammar_mismatches_are_fail_closed() {
        let bad_number = "/s/a.py\n    x: DOC101: msg\n";
        assert!(parse_pydoclint(bad_number.as_bytes(), Some(1), &["/s/a.py"]).is_err());
        let bad_rule = "/s/a.py\n    4: DOC: msg\n";
        assert!(parse_pydoclint(bad_rule.as_bytes(), Some(1), &["/s/a.py"]).is_err());
        assert!(parse_pydoclint(&[0xff], Some(1), &["/s/a.py"]).is_err());
        let blanked = "/s/a.py\n\n    4: DOC101: msg\n\n";
        let findings = parse_pydoclint(blanked.as_bytes(), Some(1), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
    }
}
