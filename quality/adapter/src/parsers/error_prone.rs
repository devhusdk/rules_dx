use super::{
    check_output_size, finding, known, missing, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;

fn error_prone_rule(rule: &str) -> bool {
    let mut chars = rule.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => (),
        _ => return false,
    }
    rule.chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn is_summary(line: &str) -> bool {
    let Some((count, rest)) = line.split_once(' ') else {
        return false;
    };
    if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    matches!(rest, "error" | "errors" | "warning" | "warnings")
}

fn error_prone_header(line: &str) -> Result<(&str, u64, ToolSeverity, &str, String), ParseError> {
    const TOOL: &str = "error_prone";
    let (path, rest) = line
        .split_once(':')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    if path.is_empty() {
        return Err(missing(TOOL, "diagnostic", line));
    }
    let (line_text, tail) = rest
        .split_once(':')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let line_no: u64 = line_text.parse().map_err(|_| ParseError::Shape {
        tool: TOOL,
        detail: format!("malformed diagnostic: {line:?}"),
    })?;
    if line_no < 1 {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed diagnostic: {line:?}"),
        });
    }
    let tail = tail
        .strip_prefix(' ')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    let (severity_text, body) = tail
        .split_once(':')
        .ok_or_else(|| missing(TOOL, "diagnostic", line))?;
    if severity_text.bytes().all(|byte| byte.is_ascii_digit()) && !severity_text.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("javac column form is not pinned: {line:?}"),
        });
    }
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
    let body = body.strip_prefix(' ').unwrap_or(body);
    if body.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed diagnostic: {line:?}"),
        });
    }
    let (rule, message) = match body.strip_prefix('[') {
        Some(bracketed) => {
            let (rule, message) = bracketed
                .split_once(']')
                .ok_or_else(|| missing(TOOL, "check", line))?;
            if !error_prone_rule(rule) {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("unknown check in {line:?}"),
                });
            }
            let message = message.strip_prefix(' ').unwrap_or(message);
            if message.is_empty() {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("malformed diagnostic: {line:?}"),
                });
            }
            (rule, message.to_owned())
        }
        None => ("javac", body.to_owned()),
    };
    Ok((path, line_no, severity, rule, message))
}

pub fn parse_error_prone(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "error_prone";
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let out_text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if !out_text.trim().is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "unexpected stdout output".to_owned(),
        });
    }
    let log = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in log.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        if line.starts_with("Note:") || line.starts_with("Note ") {
            continue;
        }
        if is_summary(line) {
            continue;
        }
        let (path, line_no, severity, rule, message) = error_prone_header(line)?;
        let file = known(TOOL, files, path)?;
        findings.push(finding(
            TOOL,
            file,
            rule.to_owned(),
            message,
            severity,
            line_no,
            1,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const ERROR_PRONE_DIRTY: &str = concat!(
        "/s/Hello.java:3: error: [DeadException] Exception created but not thrown\n",
        "    new IllegalArgumentException(\"Missing required argument\");\n",
        "    ^\n",
        "    (see https://errorprone.info/bugpattern/DeadException)\n",
        "  Did you mean 'throw new IllegalArgumentException(\"Missing required argument\");'?\n",
        "/s/Hello.java:7: warning: [MissingOverride] Method overrides Object.equals without @Override\n",
        "    public boolean equals(Object other) {\n",
        "    ^\n",
        "    (see https://errorprone.info/bugpattern/MissingOverride)\n",
        "1 error\n",
        "1 warning\n",
    );

    #[test]
    fn error_prone_reports_bracketed_checks_as_line_points() {
        let findings = parse_error_prone(
            b"",
            ERROR_PRONE_DIRTY.as_bytes(),
            Some(1),
            &["/s/Hello.java"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/Hello.java");
        assert_eq!(findings[0].finding.tool_id, "error_prone");
        assert_eq!(findings[0].finding.rule_id, "DeadException");
        assert_eq!(
            findings[0].finding.message,
            "Exception created but not thrown"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 3, column: 1 }, None)
        );
        assert_eq!(findings[1].finding.rule_id, "MissingOverride");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[1].finding.start, findings[1].finding.end),
            (TextPosition { line: 7, column: 1 }, None)
        );
        assert!(findings
            .iter()
            .all(|finding| finding.finding.suggestions.is_empty()));
        assert!(parse_error_prone(b"", b"", Some(0), &["/s/Hello.java"])
            .expect("parsed")
            .is_empty());
        assert!(parse_error_prone(b"", b"", Some(1), &["/s/Hello.java"]).is_err());
        assert!(parse_error_prone(
            b"",
            ERROR_PRONE_DIRTY.as_bytes(),
            Some(1),
            &["/s/other.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"chatter",
            ERROR_PRONE_DIRTY.as_bytes(),
            Some(1),
            &["/s/Hello.java"],
        )
        .is_err());
    }

    #[test]
    fn error_prone_reports_unbracketed_javac_headers() {
        let stderr = concat!(
            "/s/Broken.java:5: error: cannot find symbol\n",
            "    symbol:   variable missing\n",
            "    location: class hello.Broken\n",
            "Note: Some input files use unchecked or unsafe operations.\n",
            "1 error\n",
        );
        let findings = parse_error_prone(b"", stderr.as_bytes(), Some(1), &["/s/Broken.java"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "javac");
        assert_eq!(findings[0].finding.message, "cannot find symbol");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 5, column: 1 }, None)
        );
    }

    #[test]
    fn error_prone_survives_colons_inside_the_message() {
        let stderr =
            "/s/A.java:9: warning: [DefaultCharset] Implicit use of the platform default charset: UTF-8\n";
        let findings =
            parse_error_prone(b"", stderr.as_bytes(), Some(0), &["/s/A.java"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "DefaultCharset");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.message,
            "Implicit use of the platform default charset: UTF-8"
        );
    }

    #[test]
    fn error_prone_grammar_mismatches_are_fail_closed() {
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:3:14: error: [DeadException] msg\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:3: note: [DeadException] msg\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:3: error: [] msg\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:3: error: [bad-name] msg\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:3: error: [DeadException]\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(
            b"",
            b"/s/A.java:0: error: [DeadException] msg\n",
            Some(1),
            &["/s/A.java"],
        )
        .is_err());
        assert!(parse_error_prone(b"", b"garbage\n", Some(1), &["/s/A.java"],).is_err());
        assert!(parse_error_prone(&[0xff], &[0xff], Some(1), &["/s/A.java"]).is_err());
        let stderr =
            "2 errors\n3 warnings\n\n\tindented\n/s/A.java:1: error: [DeadException] msg\n";
        let findings =
            parse_error_prone(b"", stderr.as_bytes(), Some(1), &["/s/A.java"]).expect("parsed");
        assert_eq!(findings.len(), 1);
    }
}
