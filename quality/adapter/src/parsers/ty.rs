use super::{finding, known, lines, missing, require_findings, FileFinding, ParseError};
use crate::ToolSeverity;

fn ty_diagnostic(line: &str) -> Result<(&str, u64, u64, ToolSeverity, &str, String), ParseError> {
    const TOOL: &str = "ty";
    let mut parts = line.splitn(4, ':');
    let (path, line_text, column_text, tail) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    if path.is_empty() || tail.is_empty() {
        return Err(missing(TOOL, "diagnostic", line));
    }
    let (line_no, column) = (line_text.parse::<u64>(), column_text.parse::<u64>());
    let tail = tail
        .strip_prefix(' ')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let (severity_text, rest) = tail
        .split_once('[')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let (rule, message) = rest
        .split_once(']')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let severity = match severity_text {
        "error" => ToolSeverity::Error,
        "warning" => ToolSeverity::Warning,
        _ => {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unknown severity in {line:?}"),
            });
        }
    };
    let message = message.strip_prefix(' ').unwrap_or(message);
    match (
        path.is_empty(),
        line_no,
        column,
        rule.is_empty(),
        message.is_empty(),
    ) {
        (false, Ok(line_no), Ok(column), false, false) if line_no >= 1 && column >= 1 => {
            Ok((path, line_no, column, severity, rule, message.to_owned()))
        }
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed diagnostic: {line:?}"),
        }),
    }
}

pub fn parse_ty(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "ty";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        if line == "All checks passed!" {
            continue;
        }
        if line.starts_with("Found ")
            && (line.ends_with("diagnostic") || line.ends_with("diagnostics"))
        {
            continue;
        }
        let (path, line_no, column, severity, rule, message) = ty_diagnostic(line)?;
        let file = known(TOOL, files, path)?;
        findings.push(finding(
            TOOL,
            file,
            rule.to_owned(),
            message,
            severity,
            line_no,
            column,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    #[test]
    fn ty_reports_concise_diagnostics() {
        let stdout = concat!(
            "/s/dirty.py:1:10: error[invalid-assignment] Object of type `Literal[\"hello\"]` is not assignable to `int`\n",
            "Found 1 diagnostic\n",
        );
        let findings = parse_ty(stdout.as_bytes(), Some(1), &["/s/dirty.py"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.py");
        assert_eq!(findings[0].finding.tool_id, "ty");
        assert_eq!(findings[0].finding.rule_id, "invalid-assignment");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition {
                    line: 1,
                    column: 10
                },
                None
            )
        );
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_ty(b"All checks passed!\n", Some(0), &["/s/dirty.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_ty(b"All checks passed!\n", Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_ty(b"garbage\n", Some(1), &["/s/dirty.py"]).is_err());
        assert!(parse_ty(stdout.as_bytes(), Some(1), &["/s/other.py"]).is_err());
    }

    #[test]
    fn ty_survives_colons_inside_the_message() {
        let stdout = concat!(
            "quality/testdata/real_dirty.py:22:18: error[invalid-argument-type] Argument to function `add` is incorrect: Expected `int`, found `Literal[\"two\"]`\n",
            "Found 1 diagnostic\n",
        );
        let findings = parse_ty(
            stdout.as_bytes(),
            Some(1),
            &["quality/testdata/real_dirty.py"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "invalid-argument-type");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition {
                    line: 22,
                    column: 18
                },
                None
            )
        );
    }

    #[test]
    fn ty_grammar_mismatches_are_fail_closed() {
        assert!(parse_ty(b"/s/a.py:1:1: info[rule] msg\n", Some(1), &["/s/a.py"]).is_err());
        assert!(parse_ty(b"/s/a.py:0:1: error[rule] msg\n", Some(1), &["/s/a.py"]).is_err());
        assert!(parse_ty(&[0xff], Some(1), &["/s/a.py"]).is_err());
    }
}
