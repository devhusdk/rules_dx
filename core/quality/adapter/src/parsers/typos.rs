use serde::Deserialize;

use super::{FileFinding, ParseError, check_output_size, code_name, finding, known};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

const TOOL: &str = "typos";

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "type")]
    kind: String,
    path: Option<String>,
    line_num: Option<u64>,
    byte_offset: Option<u64>,
    typo: Option<String>,
    corrections: Option<Vec<String>>,
}

/// `typos --format json` prints one JSON object per line. It exits 0 when nothing is wrong, 2
/// when it found typos and anything else on a usage or file error.
pub fn parse_typos(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if !matches!(code, Some(0 | 2)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let text =
        std::str::from_utf8(stdout).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let mut findings = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let record: Record = serde_json::from_str(line).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        match record.kind.as_str() {
            "typo" => {}
            "binary_file" => continue,
            other => {
                return Err(ParseError::shape(
                    TOOL,
                    format!("unknown record type: {other}"),
                ));
            }
        }
        let missing = |what: &str| ParseError::shape(TOOL, format!("typo record without {what}"));
        let path = record.path.ok_or_else(|| missing("path"))?;
        let typo = record.typo.ok_or_else(|| missing("typo"))?;
        let (position, name_typo) = match record.line_num {
            None => (TextPosition { line: 1, column: 1 }, true),
            Some(0) => return Err(missing("line")),
            Some(line) => {
                let offset = record.byte_offset.ok_or_else(|| missing("offset"))?;
                (
                    TextPosition {
                        line,
                        column: offset + 1,
                    },
                    false,
                )
            }
        };
        let prefix = if name_typo { "file name: " } else { "" };
        let corrections = record.corrections.unwrap_or_default();
        let message = if corrections.is_empty() {
            format!("{prefix}`{typo}` is a typo")
        } else {
            format!("{prefix}`{typo}` should be {}", quoted(&corrections))
        };
        findings.push(finding(
            TOOL,
            known(TOOL, files, &path)?,
            String::new(),
            message,
            ToolSeverity::Warning,
            position,
            ColumnEncoding::Utf8,
        ));
    }
    if findings.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} typos", code_name(code), findings.len()),
        ));
    }
    Ok(findings)
}

fn quoted(corrections: &[String]) -> String {
    corrections
        .iter()
        .map(|word| format!("`{word}`"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = concat!(
        "{\"type\":\"typo\",\"path\":\"a.txt\",\"line_num\":2,\"byte_offset\":4,",
        "\"typo\":\"recieve\",\"corrections\":[\"receive\"]}\n"
    );

    #[test]
    fn typos_reports_the_word_and_its_correction() {
        let found = parse_typos(DIRTY.as_bytes(), Some(2), &["a.txt"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "a.txt");
        assert_eq!(found[0].finding.message, "`recieve` should be `receive`");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 5);
    }

    #[test]
    fn typos_clean_and_binary_files_report_nothing() {
        assert!(
            parse_typos(b"", Some(0), &["a.txt"])
                .expect("parsed")
                .is_empty()
        );
        let binary = "{\"type\":\"binary_file\",\"path\":\"a.txt\"}\n";
        assert!(
            parse_typos(binary.as_bytes(), Some(0), &["a.txt"])
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn typos_fails_closed() {
        assert!(parse_typos(b"", Some(2), &["a.txt"]).is_err());
        assert!(parse_typos(b"", Some(64), &["a.txt"]).is_err());
        assert!(parse_typos(b"", None, &["a.txt"]).is_err());
        assert!(parse_typos(DIRTY.as_bytes(), Some(0), &["a.txt"]).is_err());
        assert!(parse_typos(DIRTY.as_bytes(), Some(2), &["b.txt"]).is_err());
        assert!(parse_typos(b"{<error", Some(2), &["a.txt"]).is_err());
        assert!(parse_typos(b"{\"type\":\"other\"}\n", Some(2), &["a.txt"]).is_err());
        assert!(parse_typos(&[0xff], Some(2), &["a.txt"]).is_err());
        let zero = DIRTY.replace("\"line_num\":2", "\"line_num\":0");
        assert!(parse_typos(zero.as_bytes(), Some(2), &["a.txt"]).is_err());
    }

    const NAME_TYPO: &str = concat!(
        "{\"type\":\"typo\",\"path\":\"docs/recieve.md\",\"byte_offset\":0,",
        "\"typo\":\"recieve\",\"corrections\":[\"receive\"]}\n"
    );

    #[test]
    fn typos_reports_file_name_typo_at_first_line() {
        let found =
            parse_typos(NAME_TYPO.as_bytes(), Some(2), &["docs/recieve.md"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "docs/recieve.md");
        assert_eq!(
            found[0].finding.message,
            "file name: `recieve` should be `receive`"
        );
        assert_eq!(found[0].finding.start.line, 1);
        assert_eq!(found[0].finding.start.column, 1);
    }

    #[test]
    fn typos_file_name_typo_does_not_drop_others() {
        let both = format!("{NAME_TYPO}{DIRTY}");
        let found =
            parse_typos(both.as_bytes(), Some(2), &["a.txt", "docs/recieve.md"]).expect("parsed");
        assert_eq!(found.len(), 2);
    }
}
