use super::{
    check_output_size, code_name, finding, known, unformatted_at, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_prettier_check(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "prettier";
    check_output_size(TOOL, stderr)?;
    let text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "The system cannot find the path specified." {
            continue;
        }
        if let Some(error) = trimmed.strip_prefix("[error]") {
            syntax_error(error.trim_start(), files, &mut findings)?;
            continue;
        }
        let Some(path) = trimmed.strip_prefix("[warn] ") else {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unexpected stderr line: {trimmed:?}"),
            });
        };
        if path.starts_with("Code style issues") {
            continue;
        }
        let normalized = path.strip_prefix("./").unwrap_or(path);
        let checked = known(TOOL, files, normalized)?;
        findings.push(unformatted_at(TOOL, checked, 1, 1));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no warn lines", code_name(code)),
        });
    }
    Ok(findings)
}

fn syntax_error(
    text: &str,
    files: &[&str],
    findings: &mut Vec<FileFinding>,
) -> Result<(), ParseError> {
    const TOOL: &str = "prettier";
    let named = files.iter().find_map(|file| {
        let message = text
            .strip_prefix(file)
            .or_else(|| text.strip_prefix("./").and_then(|t| t.strip_prefix(file)))?
            .strip_prefix(": ")?;
        Some((*file, message))
    });
    let Some((file, message)) = named else {
        let is_frame = text.contains('|') && !text.contains("Error");
        return match findings.last_mut() {
            Some(last) if last.finding.severity == ToolSeverity::Error => {
                if !is_frame && !text.is_empty() {
                    last.finding.message.push('\n');
                    last.finding.message.push_str(text);
                }
                Ok(())
            }
            _ => Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("error without a file: {text:?}"),
            }),
        };
    };
    let (line, column) = error_position(message).unwrap_or((1, 1));
    findings.push(finding(
        TOOL,
        file,
        String::new(),
        message.trim().to_owned(),
        ToolSeverity::Error,
        line,
        column,
    ));
    Ok(())
}

fn error_position(message: &str) -> Option<(u64, u64)> {
    let head = message.lines().next()?.trim_end();
    let tail = head
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once('('))
        .map(|(_, position)| position)
        .or_else(|| head.rsplit_once(" at ").map(|(_, position)| position))?;
    let (line, column) = tail.split_once(':')?;
    Some((line.trim().parse().ok()?, column.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prettier_check_reports_warn_lines() {
        let stderr = "[warn] src/a.js\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/a.js");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_prettier_check(b"", Some(0), &["src/a.js"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_prettier_check(b"", Some(1), &["src/a.js"]).is_err());
        assert!(parse_prettier_check(b"unexpected\n", Some(1), &["src/a.js"]).is_err());
        assert!(parse_prettier_check(stderr.as_bytes(), Some(1), &["src/other.js"]).is_err());
        assert!(parse_prettier_check(&[0xff], Some(1), &["src/a.js"]).is_err());
        let padded = "[warn] src/a.js\n\n";
        let findings =
            parse_prettier_check(padded.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let windows_noise =
            "The system cannot find the path specified.\n[warn] src/a.js\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(windows_noise.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean_noise = parse_prettier_check(
            b"The system cannot find the path specified.\n",
            Some(0),
            &["src/a.js"],
        )
        .expect("parsed");
        assert!(clean_noise.is_empty());
    }

    #[test]
    fn prettier_check_locates_a_syntax_error_beside_unformatted_files() {
        let stderr = "[error] a/bad.json5: SyntaxError: JSON5: invalid end of input at 2:1\n[error] > 1 | { \"a\": [1,\n[error]     |           ^\n[warn] a/dirty.html\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(2), &["a/bad.json5", "a/dirty.html"])
                .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "a/bad.json5");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(findings[0].finding.message.contains("invalid end of input"));
        assert_eq!(findings[1].file, "a/dirty.html");
    }

    #[test]
    fn prettier_check_reads_a_parenthesised_position() {
        let stderr = "[error] x.graphql: SyntaxError: Expected Name, found <EOF>. (1:17)\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(2), &["x.graphql"]).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 17);
        assert!(parse_prettier_check(b"[error] other: boom\n", Some(2), &["x.graphql"]).is_err());
    }
}
