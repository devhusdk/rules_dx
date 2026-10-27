use super::{as_text, code_name, known_spelled, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "deadnix";

/// How deadnix spells an IO failure in a result message.
const IO_ERROR: &str = "(os error ";

fn number(value: &serde_json::Value, key: &str) -> Result<Option<u64>, ParseError> {
    match value.get(key) {
        None => Ok(None),
        Some(raw) => raw
            .as_u64()
            .map(|number| Some(number.max(1)))
            .ok_or_else(|| ParseError::shape(TOOL, format!("{key} is not a number"))),
    }
}

/// Parses `deadnix --fail -o json <files>`: one JSON object per file with results.
///
/// A result with a line is dead code. A result without one is a parse or IO failure, which
/// deadnix reports with exit 0 even under `--fail`: a parse failure becomes a `syntax` error
/// finding and an IO failure fails closed.
pub fn parse_deadnix(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {stderr}", code_name(code)),
        ));
    }
    let report = as_text(TOOL, stdout)?.trim();
    let mut findings = Vec::new();
    let mut dead = false;
    for line in report
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !line.starts_with('{') {
            return Err(ParseError::Json {
                tool: TOOL,
                detail: format!(
                    "stdout must be one JSON object per line, got {:?}",
                    line.chars().take(80).collect::<String>()
                ),
            });
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|err| ParseError::Json {
                tool: TOOL,
                detail: err.to_string(),
            })?;
        let file = value
            .get("file")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ParseError::shape(TOOL, "missing file"))?;
        let checked = known_spelled(TOOL, files, file)?;
        let results = value
            .get("results")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| ParseError::shape(TOOL, "missing results"))?;
        for result in results {
            let message = result
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .ok_or_else(|| ParseError::shape(TOOL, "missing message"))?;
            let (rule_id, severity, start, end) = match number(result, "line")? {
                Some(line) => {
                    dead = true;
                    let column = number(result, "column")?
                        .ok_or_else(|| ParseError::shape(TOOL, "missing column"))?;
                    let end = number(result, "endColumn")?
                        .filter(|end| *end > column)
                        .map(|column| TextPosition { line, column });
                    let rule_id = message
                        .split_once(':')
                        .map_or("unused", |(kind, _)| kind)
                        .to_ascii_lowercase()
                        .replace(' ', "_");
                    (
                        rule_id,
                        ToolSeverity::Warning,
                        TextPosition { line, column },
                        end,
                    )
                }
                None if message.contains(IO_ERROR) => {
                    return Err(ParseError::Incomplete {
                        tool: TOOL,
                        detail: format!("deadnix could not read {checked}: {message}"),
                    });
                }
                None => (
                    "syntax".to_owned(),
                    ToolSeverity::Error,
                    TextPosition { line: 1, column: 1 },
                    None,
                ),
            };
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id,
                    message: message.to_owned(),
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf8,
                    related: Vec::new(),
                },
            });
        }
    }
    if code == Some(1) && !dead {
        return Err(ParseError::shape(
            TOOL,
            format!("exit 1 without dead code: {stderr}"),
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = r#"{"file":"dirty.nix","results":[{"column":47,"endColumn":48,"line":4,"message":"Unused let binding: a"}]}"#;
    const SYNTAX: &str = r#"{"file":"syntax.nix","results":[{"message":"unexpected TOKEN_SEMICOLON at 6..7"},{"message":"unexpected end of file"}]}"#;

    #[test]
    fn deadnix_clean_run_is_clean() {
        assert!(parse_deadnix(b"", b"", Some(0), &["a.nix"])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn deadnix_reports_dead_code() {
        let findings =
            parse_deadnix(DIRTY.as_bytes(), b"", Some(1), &["dirty.nix"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0].finding;
        assert_eq!(finding.rule_id, "unused_let_binding");
        assert_eq!(finding.severity, ToolSeverity::Warning);
        assert_eq!(finding.message, "Unused let binding: a");
        assert_eq!(
            finding.start,
            TextPosition {
                line: 4,
                column: 47
            }
        );
        assert_eq!(
            finding.end,
            Some(TextPosition {
                line: 4,
                column: 48
            })
        );
    }

    #[test]
    fn deadnix_parse_errors_are_error_findings_even_with_exit_zero() {
        let findings =
            parse_deadnix(SYNTAX.as_bytes(), b"", Some(0), &["syntax.nix"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .all(|f| f.finding.rule_id == "syntax" && f.finding.severity == ToolSeverity::Error));
        let both = format!("{DIRTY}\n{SYNTAX}\n");
        let findings = parse_deadnix(both.as_bytes(), b"", Some(1), &["dirty.nix", "syntax.nix"])
            .expect("parsed");
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn deadnix_io_errors_fail_closed() {
        let unreadable =
            br#"{"file":"a.nix","results":[{"message":"Permission denied (os error 13)"}]}"#;
        let err = parse_deadnix(unreadable, b"", Some(0), &["a.nix"]).expect_err("io");
        assert!(err.to_string().contains("Permission denied"), "{err}");
    }

    #[test]
    fn deadnix_rejects_malformed_output_and_exit_codes() {
        assert!(parse_deadnix(b"{\"file\":", b"", Some(1), &["a.nix"]).is_err());
        assert!(parse_deadnix(b"a.nix: unused\n", b"", Some(1), &["a.nix"]).is_err());
        assert!(parse_deadnix(b"", b"", Some(1), &["a.nix"]).is_err());
        assert!(parse_deadnix(DIRTY.as_bytes(), b"", Some(2), &["dirty.nix"]).is_err());
        assert!(parse_deadnix(DIRTY.as_bytes(), b"", None, &["dirty.nix"]).is_err());
        assert!(parse_deadnix(SYNTAX.as_bytes(), b"", Some(1), &["syntax.nix"]).is_err());
        assert!(parse_deadnix(&[0xff], b"", Some(0), &["a.nix"]).is_err());
        let no_column =
            r#"{"file":"dirty.nix","results":[{"line":4,"message":"Unused let binding: a"}]}"#;
        assert!(parse_deadnix(no_column.as_bytes(), b"", Some(1), &["dirty.nix"]).is_err());
    }

    #[test]
    fn deadnix_rejects_unknown_files() {
        let err = parse_deadnix(DIRTY.as_bytes(), b"", Some(1), &["other.nix"])
            .expect_err("unknown file");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
