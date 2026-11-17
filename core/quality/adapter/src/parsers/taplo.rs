use super::{
    FileFinding, ParseError, check_output_size, code_name, finding, known, missing, unformatted_at,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

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
        if let Some((_, after)) = trimmed.split_once("┌─ ") {
            let location = after.trim();
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
                if let Some(block) = blocks.get_mut(index)
                    && !block.message.ends_with(detail)
                {
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

/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
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
            TextPosition {
                line: block.line,
                column: block.column,
            },
            ColumnEncoding::Scalars,
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

/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
pub fn parse_taplo_format_check(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "taplo";
    check_output_size(TOOL, stderr)?;
    let mut findings = parse_taplo_lint(stderr, Some(0), files)?;
    if code == Some(0) {
        return Ok(findings);
    }
    if code != Some(1) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} is not a format check result", code_name(code)),
        });
    }
    if files
        .iter()
        .all(|file| findings.iter().any(|found| found.file == *file))
    {
        return Ok(findings);
    }
    match files {
        [only] => {
            let checked = known(TOOL, files, only)?;
            findings.push(unformatted_at(TOOL, checked, 1, 1, ColumnEncoding::Scalars));
            Ok(findings)
        }
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "exit {} across {} files names no file for every input",
                code_name(code),
                files.len()
            ),
        }),
    }
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
        let actual =
            parse_taplo_lint(b" INFO collect\n", Some(0), &["/s/clean.toml"]).expect("parsed");
        assert!(actual.is_empty(), "{actual:?}");
        assert!(parse_taplo_lint(b" INFO collect\n", Some(1), &["/s/clean.toml"]).is_err());
    }

    #[test]
    fn taplo_format_check_adds_unformatted_files() {
        for stderr in [
            &b" INFO collect\nERROR taplo:format_files: the file is not properly formatted path=\"/s/fmt.toml\"\n"[..],
            b" INFO collect\n",
            b"",
        ] {
            let findings =
                parse_taplo_format_check(stderr, Some(1), &["/s/fmt.toml"]).expect("parsed");
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].file, "/s/fmt.toml");
            assert_eq!(findings[0].finding.message, "file is not formatted");
            assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        }
        let findings =
            parse_taplo_format_check(TAPLO_DIRTY_STDERR.as_bytes(), Some(1), &["/s/dirty.toml"])
                .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(
            parse_taplo_format_check(b" INFO collect\n", Some(0), &["/s/fmt.toml"])
                .expect("parsed")
                .is_empty()
        );
        assert!(
            parse_taplo_format_check(b" INFO collect\n", Some(1), &["/s/a.toml", "/s/b.toml"])
                .is_err()
        );
        for code in [None, Some(2), Some(-1)] {
            assert!(parse_taplo_format_check(b" INFO collect\n", code, &["/s/fmt.toml"]).is_err());
        }
    }

    #[test]
    fn taplo_format_check_mixed_batch_defers_to_per_file_recheck() {
        let marker = b" INFO collect\nERROR taplo:format_files: the file is not properly formatted path=\"/s/fmt.toml\"\nERROR operation failed\n";
        let mixed = [TAPLO_DIRTY_STDERR.as_bytes(), &marker[..]].concat();
        assert!(
            parse_taplo_format_check(&mixed, Some(1), &["/s/dirty.toml", "/s/fmt.toml"]).is_err()
        );
        let both = [
            TAPLO_DIRTY_STDERR.as_bytes(),
            TAPLO_DIRTY_STDERR
                .replace("dirty.toml", "other.toml")
                .as_bytes(),
        ]
        .concat();
        let findings =
            parse_taplo_format_check(&both, Some(1), &["/s/dirty.toml", "/s/other.toml"])
                .expect("parsed");
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn taplo_rejects_orphan_blocks_bad_locations_and_bytes() {
        assert!(
            parse_taplo_lint(
                "  \u{250c}\u{2500} /s/x.toml:1:5\n".as_bytes(),
                Some(1),
                &["/s/x.toml"]
            )
            .is_err()
        );
        assert!(
            parse_taplo_lint(
                "error: bad\n  \u{250c}\u{2500} /s/x.toml:a:b\n".as_bytes(),
                Some(1),
                &["/s/x.toml"]
            )
            .is_err()
        );
        assert!(parse_taplo_lint(b"\xff", Some(1), &["/s/x.toml"]).is_err());
        assert!(parse_taplo_format_check(b"\xff", Some(1), &["/s/x.toml"]).is_err());
    }
}
