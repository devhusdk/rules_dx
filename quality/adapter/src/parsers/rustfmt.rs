use super::{
    check_output_size, code_name, finding, known, missing, unformatted_at, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_rustfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "rustfmt";
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let stdout_text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let stderr_text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in stdout_text.lines() {
        if let Some(header) = line.strip_prefix("Diff in ") {
            let (path, line) = rustfmt_header(header)?;
            let checked = known(TOOL, files, path)?;
            findings.push(unformatted_at(TOOL, checked, line, 1));
        }
    }
    let mut pending: Option<String> = None;
    for line in stderr_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("error") {
            pending = Some(trimmed.to_owned());
            continue;
        }
        if let Some(arrow) = trimmed.strip_prefix("--> ") {
            if let Some(message) = pending.take() {
                let (path, line, column) = rustfmt_location(arrow)?;
                let checked = known(TOOL, files, path)?;
                findings.push(finding(
                    TOOL,
                    checked,
                    String::new(),
                    message,
                    ToolSeverity::Error,
                    line,
                    column,
                ));
            }
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "exit {} with no Diff headers or error blocks",
                code_name(code)
            ),
        });
    }
    Ok(findings)
}

fn rustfmt_header(header: &str) -> Result<(&str, u64), ParseError> {
    let (rest, tail) = header
        .rsplit_once(':')
        .ok_or_else(|| missing("rustfmt", "Diff header", header))?;
    if !tail.is_empty() {
        return Err(ParseError::Shape {
            tool: "rustfmt",
            detail: format!("malformed Diff header: {header}"),
        });
    }
    let (path, line_text) = rest
        .rsplit_once(':')
        .ok_or_else(|| missing("rustfmt", "Diff header", header))?;
    let line = line_text.parse::<u64>().map_err(|_| ParseError::Shape {
        tool: "rustfmt",
        detail: format!("malformed Diff header: {header}"),
    })?;
    if path.is_empty() || line == 0 {
        return Err(ParseError::Shape {
            tool: "rustfmt",
            detail: format!("malformed Diff header: {header}"),
        });
    }
    Ok((path, line))
}

fn rustfmt_location(arrow: &str) -> Result<(&str, u64, u64), ParseError> {
    let (rest, column_text) = arrow
        .rsplit_once(':')
        .ok_or_else(|| missing("rustfmt", "error location", arrow))?;
    let (path, line_text) = rest
        .rsplit_once(':')
        .ok_or_else(|| missing("rustfmt", "error location", arrow))?;
    let (line, column) = (line_text.parse::<u64>(), column_text.parse::<u64>());
    match (path.is_empty(), line, column) {
        (false, Ok(line), Ok(column)) if line >= 1 && column >= 1 => Ok((path, line, column)),
        _ => Err(ParseError::Shape {
            tool: "rustfmt",
            detail: format!("malformed error location: {arrow}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    #[test]
    fn rustfmt_reports_diff_headers_and_syntax_blocks() {
        let dirty = parse_rustfmt(
            b"Diff in /s/dirty.rs:1:\n-fn  main( ){}\n+fn main() {}\n",
            b"",
            Some(1),
            &["/s/dirty.rs"],
        )
        .expect("parsed");
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].finding.message, "file is not formatted");
        assert_eq!(dirty[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(dirty[0].finding.start, TextPosition { line: 1, column: 1 });

        let broken = parse_rustfmt(
            b"",
            b"error: this file contains an unclosed delimiter\n --> /s/broken.rs:1:12\n  |\n",
            Some(1),
            &["/s/broken.rs"],
        )
        .expect("parsed");
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            broken[0].finding.start,
            TextPosition {
                line: 1,
                column: 12
            }
        );
        assert!(broken[0].finding.message.starts_with("error:"));

        assert!(parse_rustfmt(b"", b"", Some(0), &["/s/clean.rs"])
            .expect("parsed")
            .is_empty());
        assert!(parse_rustfmt(b"", b"", Some(1), &["/s/clean.rs"]).is_err());
        assert!(
            parse_rustfmt(b"Diff in /s/other.rs:1:\n", b"", Some(1), &["/s/clean.rs"]).is_err()
        );
    }

    #[test]
    fn rustfmt_rejects_bytes_headers_and_locations() {
        assert!(parse_rustfmt(b"\xff", b"", Some(1), &["/s/x.rs"]).is_err());
        assert!(parse_rustfmt(b"", b"\xff", Some(1), &["/s/x.rs"]).is_err());
        for header in [
            "Diff in nocolon",
            "Diff in /s/x.rs:1:extra",
            "Diff in /s/x.rs:abc:",
            "Diff in :0:",
        ] {
            assert!(
                parse_rustfmt(header.as_bytes(), b"", Some(1), &["/s/x.rs"]).is_err(),
                "header: {header}"
            );
        }
        assert!(
            parse_rustfmt(b"", b" --> /s/x.rs:1:1\n", Some(0), &["/s/x.rs"])
                .expect("parsed")
                .is_empty()
        );
        assert!(parse_rustfmt(
            b"",
            b"error: boom\n --> /s/x.rs:a:b\n",
            Some(1),
            &["/s/x.rs"]
        )
        .is_err());
    }
}
