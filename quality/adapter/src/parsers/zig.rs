use super::{finding, FileFinding, ParseError, ToolSeverity};

pub fn parse_zig_fmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if code == Some(0) {
        return Ok(Vec::new());
    }
    if code != Some(1) {
        return Err(ParseError::Shape {
            tool: "zig fmt",
            detail: "unexpected exit code".to_owned(),
        });
    }
    let mut findings = Vec::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        let path = line.trim();
        if path.is_empty() || !files.contains(&path) {
            continue;
        }
        findings.push(finding(
            "zig",
            path,
            "zig-fmt".to_owned(),
            "zig fmt would rewrite this file".to_owned(),
            ToolSeverity::Warning,
            1,
            1,
        ));
    }
    if findings.is_empty() {
        return Err(ParseError::Shape {
            tool: "zig fmt",
            detail: "exit 1 with no reported files".to_owned(),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zig_fmt_reports_listed_files() {
        let findings =
            parse_zig_fmt(b"a.zig\nb.zig\n", Some(1), &["a.zig", "b.zig"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "a.zig");
        assert_eq!(findings[0].finding.rule_id, "zig-fmt");
    }

    #[test]
    fn zig_fmt_clean_is_empty() {
        assert!(parse_zig_fmt(b"", Some(0), &["a.zig"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn zig_fmt_rejects_unexpected_exit() {
        assert!(parse_zig_fmt(b"", Some(2), &["a.zig"]).is_err());
    }

    #[test]
    fn zig_fmt_rejects_unknown_file() {
        assert!(parse_zig_fmt(b"other.zig\n", Some(1), &["a.zig"]).is_err());
    }
}
