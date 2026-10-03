use super::{
    check_output_size, code_name, finding, known, missing, unformatted_at, FileFinding, ParseError,
};
use crate::ToolSeverity;

struct TaploBlock {
    file: String,
    line: u64,
    column: u64,
    message: String,
}

fn taplo_blocks(stderr: &str) -> Result<Vec<TaploBlock>, ParseError> {
    const TOOL: &str = "taplo";
    let mut blocks: Vec<TaploBlock> = Vec::new();
    let mut pending: Option<String> = None;
    let mut open: Option<usize> = None;
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if let Some(opener) = trimmed.strip_prefix("error:") {
            pending = Some(opener.trim().to_owned());
            open = None;
            continue;
        }
        if let Some(marker) = trimmed.find("┌─ ") {
            let location = trimmed[marker + "┌─ ".len()..].trim();
            let (path, line, column) = taplo_location(location)?;
            let base = pending.clone().ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: format!("location without an error opener: {location}"),
            })?;
            blocks.push(TaploBlock {
                file: path.to_owned(),
                line,
                column,
                message: base,
            });
            open = Some(blocks.len() - 1);
            continue;
        }
        if trimmed.starts_with("ERROR ") || trimmed.starts_with("INFO ") {
            open = None;
            continue;
        }
        if let Some(index) = open {
            let caret = line.rsplit('^').next().ok_or_else(|| ParseError::Shape {
                tool: TOOL,
                detail: "caret detail vanished mid-line".to_owned(),
            })?;
            let detail = caret.trim();
            if !detail.is_empty() && !detail.contains('│') && !detail.contains('─') {
                let block = &mut blocks[index];
                if !block.message.ends_with(detail) {
                    block.message.push_str(": ");
                    block.message.push_str(detail);
                }
                open = None;
            }
        }
    }
    Ok(blocks)
}

fn taplo_location(location: &str) -> Result<(&str, u64, u64), ParseError> {
    let (rest, column_text) = location
        .rsplit_once(':')
        .ok_or_else(|| missing("taplo", "error location", location))?;
    let (path, line_text) = rest
        .rsplit_once(':')
        .ok_or_else(|| missing("taplo", "error location", location))?;
    match (
        path.is_empty(),
        line_text.parse::<u64>(),
        column_text.parse::<u64>(),
    ) {
        (false, Ok(line), Ok(column)) if line >= 1 && column >= 1 => Ok((path, line, column)),
        _ => Err(ParseError::Shape {
            tool: "taplo",
            detail: format!("malformed error location: {location}"),
        }),
    }
}

pub fn parse_taplo_lint(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "taplo";
    check_output_size(TOOL, stderr)?;
    let stderr_text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for block in taplo_blocks(stderr_text)? {
        let checked = known(TOOL, files, &block.file)?;
        findings.push(finding(
            TOOL,
            checked,
            String::new(),
            block.message,
            ToolSeverity::Error,
            block.line,
            block.column,
        ));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no parsed error blocks", code_name(code)),
        });
    }
    Ok(findings)
}

pub fn parse_taplo_format_check(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "taplo";
    check_output_size(TOOL, stderr)?;
    let stderr_text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = parse_taplo_lint(stderr, Some(0), files)?;
    for line in stderr_text.lines() {
        if let Some(marker) = line.find("the file is not properly formatted") {
            let rest = &line[marker..];
            let path = rest
                .find("path=\"")
                .and_then(|start| {
                    let quoted = &rest[start + "path=\"".len()..];
                    quoted.find('"').map(|end| &quoted[..end])
                })
                .ok_or_else(|| missing(TOOL, "format path", line))?;
            let checked = known(TOOL, files, path)?;
            findings.push(unformatted_at(TOOL, checked, 1, 1));
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no parsed findings", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const TAPLO_DIRTY_STDERR: &str = " INFO taplo:lint_files:collect_files: found files\nerror: invalid TOML\n  \u{250c}\u{2500} /s/dirty.toml:2:5\n  \u{2502}  \n2 \u{2502}   b = \n  \u{2502} \u{256d}\u{2500}\u{2500}\u{2500}\u{2500}^\n3 \u{2502} \u{2502} \n  \u{2502} \u{2570}^ expected value\n\nERROR taplo:lint_files: invalid file\n";

    #[test]
    fn taplo_lint_reports_blocks_with_caret_detail() {
        let findings = parse_taplo_lint(TAPLO_DIRTY_STDERR.as_bytes(), Some(1), &["/s/dirty.toml"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.toml");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 2, column: 5 }
        );
        assert_eq!(findings[0].finding.message, "invalid TOML: expected value");
        assert!(
            parse_taplo_lint(b" INFO collect\n", Some(0), &["/s/clean.toml"])
                .expect("parsed")
                .is_empty()
        );
        assert!(parse_taplo_lint(b" INFO collect\n", Some(1), &["/s/clean.toml"]).is_err());
    }

    #[test]
    fn taplo_format_check_adds_unformatted_files() {
        let stderr = b" INFO collect\nERROR taplo:format_files: the file is not properly formatted path=\"/s/fmt.toml\"\nERROR operation failed\n";
        let findings = parse_taplo_format_check(stderr, Some(1), &["/s/fmt.toml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let mixed = [TAPLO_DIRTY_STDERR.as_bytes(), &stderr[..]].concat();
        let findings = parse_taplo_format_check(&mixed, Some(1), &["/s/dirty.toml", "/s/fmt.toml"])
            .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert!(parse_taplo_format_check(b" INFO collect\n", Some(1), &["/s/fmt.toml"]).is_err());
    }

    #[test]
    fn taplo_rejects_orphan_blocks_bad_locations_and_bytes() {
        assert!(parse_taplo_lint(
            "  \u{250c}\u{2500} /s/x.toml:1:5\n".as_bytes(),
            Some(1),
            &["/s/x.toml"]
        )
        .is_err());
        assert!(parse_taplo_lint(
            "error: bad\n  \u{250c}\u{2500} /s/x.toml:a:b\n".as_bytes(),
            Some(1),
            &["/s/x.toml"]
        )
        .is_err());
        assert!(parse_taplo_lint(b"\xff", Some(1), &["/s/x.toml"]).is_err());
        assert!(parse_taplo_format_check(b"\xff", Some(1), &["/s/x.toml"]).is_err());
        assert!(parse_taplo_format_check(
            b"ERROR taplo:format_files: the file is not properly formatted\n",
            Some(1),
            &["/s/x.toml"]
        )
        .is_err());
    }
}
