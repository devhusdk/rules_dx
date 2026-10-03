use super::{finding, known, lines, missing, require_findings, FileFinding, ParseError};
use crate::ToolSeverity;

fn tsc_diagnostic(line: &str) -> Result<(&str, u64, u64, &str, String), ParseError> {
    const TOOL: &str = "tsc";
    let (head, tail) = line
        .split_once("): ")
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let open = head
        .rfind('(')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let (path, position) = head.split_at(open);
    let (line_text, column_text) = position[1..]
        .split_once(',')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let (severity_text, rest) = tail
        .split_once(' ')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    if severity_text != "error" {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unknown severity in {line:?}"),
        });
    }
    let (rule, message) = rest
        .split_once(": ")
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let code = rule.strip_prefix("TS").unwrap_or_default();
    let (line_no, column) = match (line_text.parse::<u64>(), column_text.parse::<u64>()) {
        (Ok(line_no), Ok(column)) if line_no >= 1 && column >= 1 => (line_no, column),
        _ => {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("malformed diagnostic: {line:?}"),
            });
        }
    };
    if path.is_empty()
        || code.is_empty()
        || !code.bytes().all(|byte| byte.is_ascii_digit())
        || message.is_empty()
    {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed diagnostic: {line:?}"),
        });
    }
    Ok((path, line_no, column, rule, message.to_owned()))
}

pub fn parse_tsc(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "tsc";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (path, line_no, column, rule, message) = tsc_diagnostic(line)?;
        let file = known(TOOL, files, path)?;
        findings.push(finding(
            TOOL,
            file,
            rule.to_owned(),
            message,
            ToolSeverity::Error,
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
    fn tsc_reports_classic_diagnostics() {
        let stdout = concat!(
            "/s/dirty.ts(5,14): error TS2322: Type 'string' is not assignable to type 'number'.\n",
            "/s/dirty.ts(6,14): error TS2322: Type 'number' is not assignable to type 'string'.\n",
        );
        let findings = parse_tsc(stdout.as_bytes(), Some(2), &["/s/dirty.ts"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/dirty.ts");
        assert_eq!(findings[0].finding.tool_id, "tsc");
        assert_eq!(findings[0].finding.rule_id, "TS2322");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition {
                    line: 5,
                    column: 14
                },
                None
            )
        );
        assert_eq!(
            findings[1].finding.message,
            "Type 'number' is not assignable to type 'string'."
        );
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_tsc(b"", Some(0), &["/s/dirty.ts"])
            .expect("parsed")
            .is_empty());
        assert!(parse_tsc(b"", Some(2), &["/s/dirty.ts"]).is_err());
        assert!(parse_tsc(stdout.as_bytes(), Some(2), &["/s/other.ts"]).is_err());
    }

    #[test]
    fn tsc_reports_syntax_errors_with_the_same_shape() {
        let stdout = "/s/syntax.ts(1,21): error TS1109: Expression expected.\n";
        let findings = parse_tsc(stdout.as_bytes(), Some(2), &["/s/syntax.ts"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "TS1109");
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition {
                    line: 1,
                    column: 21
                },
                None
            )
        );
    }

    #[test]
    fn tsc_splitting_survives_parens_and_separators_in_text() {
        let stdout = "/s/weird(name).ts(1,2): error TS1234: unexpected token \"): \" here\n";
        let findings =
            parse_tsc(stdout.as_bytes(), Some(2), &["/s/weird(name).ts"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/weird(name).ts");
        assert_eq!(findings[0].finding.rule_id, "TS1234");
        assert_eq!(findings[0].finding.message, "unexpected token \"): \" here");
    }

    #[test]
    fn tsc_grammar_mismatches_are_fail_closed() {
        assert!(parse_tsc(b"garbage\n", Some(2), &["/s/a.ts"]).is_err());
        assert!(parse_tsc(
            b"/s/a.ts(1,1): warning TS1234: msg\n",
            Some(2),
            &["/s/a.ts"]
        )
        .is_err());
        assert!(parse_tsc(b"/s/a.ts(1,1): error 1234: msg\n", Some(2), &["/s/a.ts"]).is_err());
        assert!(parse_tsc(b"/s/a.ts(0,1): error TS1234: msg\n", Some(2), &["/s/a.ts"]).is_err());
        assert!(parse_tsc(b"/s/a.ts(1,0): error TS1234: msg\n", Some(2), &["/s/a.ts"]).is_err());
        assert!(parse_tsc(b"/s/a.ts(1,1): error TS1234: \n", Some(2), &["/s/a.ts"]).is_err());
        assert!(parse_tsc(&[0xff], Some(2), &["/s/a.ts"]).is_err());
    }
}
