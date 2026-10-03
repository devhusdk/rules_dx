use super::{check_output_size, code_name, known, unformatted_at, FileFinding, ParseError};

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
}
