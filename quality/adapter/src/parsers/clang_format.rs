use super::{
    as_text, known, missing, path_row_col, require_findings, unformatted_at, FileFinding,
    ParseError,
};

const TOOL: &str = "clang_format";
const VIOLATION_TAIL: &str = "error: code should be clang-formatted [-Wclang-format-violations]";

fn violation_at(line: &str) -> Option<(&str, u64, u64)> {
    let head = line.strip_suffix(VIOLATION_TAIL)?;
    let (path, row, column) = path_row_col(head.trim_end().strip_suffix(':')?)?;
    if row < 1 || column < 1 {
        return None;
    }
    Some((path, row, column))
}

fn is_caret(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.bytes().all(|b| b == b'^')
}

pub fn parse_clang_format(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let mut ordered: Vec<(&str, u64, u64)> = Vec::new();
    let mut tail = 0u8;
    for line in text.lines() {
        if tail == 0 {
            if line.trim().is_empty() {
                continue;
            }
            match violation_at(line) {
                Some((path, row, column)) => {
                    if !ordered.iter().any(|(seen, _, _)| *seen == path) {
                        ordered.push((path, row, column));
                    }
                    tail = 1;
                }
                None => return Err(missing(TOOL, "violation", line)),
            }
        } else if tail == 1 {
            tail = 2;
        } else if is_caret(line) {
            tail = 0;
        } else {
            return Err(missing(TOOL, "caret", line));
        }
    }
    if tail != 0 {
        return Err(ParseError::shape(TOOL, "truncated violation"));
    }
    let mut findings = Vec::new();
    for (path, row, column) in ordered {
        findings.push(unformatted_at(TOOL, known(TOOL, files, path)?, row, column));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "cc/tests/fixtures/clang_format/Sample.c:1:11: error: code should be clang-formatted [-Wclang-format-violations]\nint greet( const char*name){return 0;}\n          ^\n";

    #[test]
    fn clang_format_reports_violation_files() {
        let findings = parse_clang_format(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/clang_format/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_format/Sample.c");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 11);
        let clean = parse_clang_format(b"", Some(0), &["cc/tests/fixtures/clang_format/Sample.c"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_clang_format(b"", Some(1), &["cc/tests/fixtures/clang_format/Sample.c"]).is_err()
        );
        assert!(parse_clang_format(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_clang_format(&[0xff], Some(1), &["x"]).is_err());
        assert!(parse_clang_format(
            b"cc/tests/fixtures/clang_format/Sample.c:1:11: error: code should be clang-formatted [-Wclang-format-violations]\n",
            Some(1),
            &["cc/tests/fixtures/clang_format/Sample.c"],
        )
        .is_err());
    }
}
