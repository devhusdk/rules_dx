use serde_json::Value;

use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{FileFinding, ParseError, as_text, finding, require_findings};

const TOOL: &str = "ghc";

/// `ghc -fno-code -fdiagnostics-as-json` output: one JSON diagnostic per stderr line.
pub fn parse_ghc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut found = Vec::new();
    for raw in as_text(TOOL, output)?.lines() {
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw.trim()) else {
            continue;
        };
        if !object.contains_key("version") || !object.contains_key("span") {
            continue;
        }
        let severity = match object.get("severity").and_then(Value::as_str) {
            Some("Error") => ToolSeverity::Error,
            Some("Warning") => ToolSeverity::Warning,
            _ => continue,
        };
        let message = object
            .get("message")
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let message = message.split_whitespace().collect::<Vec<_>>().join(" ");
        if message.is_empty() {
            return Err(ParseError::shape(TOOL, format!("malformed message: {raw}")));
        }
        let rule_id = match object.get("code") {
            Some(Value::Number(number)) => format!("GHC-{number}"),
            _ => String::new(),
        };
        let located = object.get("span").and_then(|span| {
            let file = span.get("file")?.as_str()?;
            let start = span.get("start")?;
            Some((
                file,
                start.get("line")?.as_u64()?,
                start.get("column")?.as_u64()?,
            ))
        });
        let Some((path, line, column)) = located else {
            if severity == ToolSeverity::Error {
                return Err(ParseError::UnknownFile {
                    tool: TOOL,
                    path: "an error without a source span".to_owned(),
                });
            }
            continue;
        };
        let name = path.strip_prefix("./").unwrap_or(path);
        let Some(file) = files.iter().copied().find(|file| *file == name) else {
            if severity == ToolSeverity::Error {
                return Err(ParseError::UnknownFile {
                    tool: TOOL,
                    path: name.to_owned(),
                });
            }
            continue;
        };
        found.push(finding(
            TOOL,
            file,
            rule_id,
            message,
            severity,
            TextPosition { line, column },
            ColumnEncoding::ScalarsTabs { width: 8 },
        ));
    }
    require_findings(TOOL, &found, code)?;
    if code != Some(0)
        && !found
            .iter()
            .any(|f| f.finding.severity == ToolSeverity::Error)
    {
        return Err(ParseError::shape(
            TOOL,
            "nonzero exit without a parsed error diagnostic".to_owned(),
        ));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "hs/A.hs";

    fn line(file: &str, severity: &str, code: &str, message: &str) -> String {
        format!(
            "{{\"version\":\"1.1\",\"ghcVersion\":\"ghc-9.10.3\",\"span\":{{\"file\":\"{file}\",\"start\":{{\"line\":3,\"column\":7}},\"end\":{{\"line\":3,\"column\":9}}}},\"severity\":\"{severity}\",\"code\":{code},\"rendered\":\"x\",\"message\":[\"{message}\"],\"hints\":[]}}\n"
        )
    }

    #[test]
    fn ghc_reports_a_type_error_with_its_code_and_position() {
        let text = line(FILE, "Error", "83865", "Couldn't match type");
        let found = parse_ghc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.rule_id, "GHC-83865");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 7);
    }

    #[test]
    fn ghc_counts_a_tab_as_eight_columns() {
        let text = line(FILE, "Error", "83865", "Couldn't match type");
        let found = parse_ghc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        let encoding = found[0].finding.encoding;
        assert_eq!(encoding, ColumnEncoding::ScalarsTabs { width: 8 });
        assert_eq!(
            crate::line_col_to_byte("\tx = undefinedVar", 1, 9, encoding),
            Some(1)
        );
    }

    #[test]
    fn ghc_reports_warnings_and_a_null_code_has_no_rule() {
        let text = format!(
            "{}{}",
            line(FILE, "Warning", "40910", "Defined but not used"),
            line(FILE, "Warning", "null", "Odd")
        );
        let found = parse_ghc(text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.rule_id, "GHC-40910");
        assert_eq!(found[1].finding.rule_id, "");
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn ghc_werror_warning_with_a_nonzero_exit_and_no_error_fails() {
        let text = line(FILE, "Warning", "40910", "Defined but not used");
        let err = parse_ghc(text.as_bytes(), Some(1), &[FILE]).expect_err("failed");
        assert!(err.to_string().contains("without a parsed error"));
    }

    #[test]
    fn ghc_drops_a_warning_outside_the_checked_files() {
        let text = format!(
            "{}{}",
            line("hs/Dep.hs", "Warning", "40910", "w"),
            line(FILE, "Error", "1", "e")
        );
        let found = parse_ghc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn ghc_error_outside_the_checked_files_fails_and_names_it() {
        let text = line("hs/Dep.hs", "Error", "1", "e");
        let err = parse_ghc(text.as_bytes(), Some(1), &[FILE]).expect_err("failed");
        assert!(err.to_string().contains("hs/Dep.hs"));
    }

    #[test]
    fn ghc_error_with_a_null_span_fails() {
        let text = "{\"version\":\"1.1\",\"span\":null,\"severity\":\"Error\",\"code\":null,\"message\":[\"boom\"]}\n";
        assert!(parse_ghc(text.as_bytes(), Some(1), &[FILE]).is_err());
    }

    #[test]
    fn ghc_nonzero_exit_without_json_fails() {
        let err = parse_ghc(b"ghc: panic\n", Some(1), &[FILE]).expect_err("failed");
        assert!(err.to_string().contains("exit 1 with no diagnostics"));
    }

    #[test]
    fn ghc_skips_non_json_lines_on_a_clean_run() {
        let found = parse_ghc(b"[1 of 1] Compiling A\n", Some(0), &[FILE]).expect("parsed");
        assert!(found.is_empty());
    }
}
