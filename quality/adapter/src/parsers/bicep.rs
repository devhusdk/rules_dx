use super::sarif::parse_sarif;
use super::{check_output_size, code_name, unformatted_at, FileFinding, ParseError};
use crate::ColumnEncoding;

const TOOL: &str = "bicep";

/// `bicep lint --diagnostics-format sarif` prints one SARIF run on stdout and exits 0 when it
/// holds no errors. Warnings carry no `level`, errors carry `error`, and `charOffset` counts
/// UTF-16 code units from 1 (`columnKind` says so). A run with no results and a nonzero exit
/// is an error, never clean.
///
/// Bicep writes the 1-based column as the region's `charOffset`, which SARIF defines as an
/// offset from the start of the file, and never writes `startColumn`; it is moved to
/// `startColumn` before the shared SARIF parser reads it.
pub fn parse_bicep_lint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    let mut log: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let runs = log.get_mut("runs").and_then(|v| v.as_array_mut());
    for run in runs.into_iter().flatten() {
        let results = run.get_mut("results").and_then(|v| v.as_array_mut());
        for result in results.into_iter().flatten() {
            let locations = result.get_mut("locations").and_then(|v| v.as_array_mut());
            for location in locations.into_iter().flatten() {
                let region = location
                    .get_mut("physicalLocation")
                    .and_then(|v| v.get_mut("region"))
                    .and_then(|v| v.as_object_mut());
                let Some(region) = region else {
                    continue;
                };
                if !region.contains_key("startColumn") {
                    if let Some(offset) = region.remove("charOffset") {
                        region.insert("startColumn".to_owned(), offset);
                    }
                }
            }
        }
    }
    let rewritten = serde_json::to_vec(&log).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    parse_sarif(TOOL, &rewritten, code, files, ColumnEncoding::Utf16)
}

/// `bicep format --stdout FILE` prints the formatted source and exits 0, also for a file that
/// does not parse. The first differing line is reported. A nonzero exit, a signal, or empty
/// output for a non-empty file is an error, never clean.
pub fn parse_bicep_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    // A file that does not parse still exits 0 and echoes the source; the BCP diagnostic is on
    // stderr. That must not read as already formatted.
    if String::from_utf8_lossy(stderr).contains("Error BCP") {
        return Err(ParseError::shape(TOOL, "file does not parse"));
    }
    if stdout.is_empty() && !input.is_empty() {
        return Err(ParseError::shape(TOOL, "empty output for a non-empty file"));
    }
    if stdout == input {
        return Ok(Vec::new());
    }
    let formatted = stdout.split(|b| *b == b'\n');
    let original = input.split(|b| *b == b'\n');
    let line = formatted
        .clone()
        .zip(original.clone())
        .position(|(left, right)| left != right)
        .map_or_else(
            || formatted.count().min(original.count()),
            |index| index + 1,
        );
    Ok(vec![unformatted_at(
        TOOL,
        file,
        line as u64,
        1,
        ColumnEncoding::Utf16,
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolSeverity;

    const FILE: &str = "/scratch/a.bicep";

    fn sarif(results: &str) -> Vec<u8> {
        format!(
            "{{\"version\":\"2.1.0\",\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"bicep\"}}}},\"results\":[{results}],\"columnKind\":\"utf16CodeUnits\"}}]}}"
        )
        .into_bytes()
    }

    fn result(rule: &str, level: &str, file: &str, line: u64, column: u64) -> String {
        let level = if level.is_empty() {
            String::new()
        } else {
            format!("\"level\":\"{level}\",")
        };
        format!(
            "{{\"ruleId\":\"{rule}\",{level}\"message\":{{\"text\":\"message {rule}\"}},\"locations\":[{{\"physicalLocation\":{{\"artifactLocation\":{{\"uri\":\"file://{file}\"}},\"region\":{{\"startLine\":{line},\"charOffset\":{column}}}}}}}]}}"
        )
    }

    #[test]
    fn format_syntax_error_on_stderr_is_an_error() {
        let src = b"param name string =\n";
        let err = b"/a.bicep(1,20) : Error BCP009: Expected a literal value.\n";
        assert!(parse_bicep_format(src, err, Some(0), src, FILE).is_err());
    }

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_bicep_lint(&sarif(""), Some(0), &[FILE])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn warning_without_level_and_error_are_located() {
        let results = format!(
            "{},{}",
            result("no-unused-params", "", FILE, 1, 7),
            result("BCP033", "error", FILE, 2, 18)
        );
        let found = parse_bicep_lint(&sarif(&results), Some(1), &[FILE]).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.rule_id, "no-unused-params");
        assert_eq!(
            (found[0].finding.start.line, found[0].finding.start.column),
            (1, 7)
        );
        assert_eq!(found[1].finding.severity, ToolSeverity::Error);
        assert_eq!(found[1].finding.encoding, ColumnEncoding::Utf16);
        assert_eq!(found[1].file, FILE);
    }

    #[test]
    fn column_after_multibyte_text_counts_utf16_units() {
        let results = result("BCP001", "error", FILE, 1, 9);
        let found = parse_bicep_lint(&sarif(&results), Some(1), &[FILE]).unwrap();
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(found[0].finding.encoding, ColumnEncoding::Utf16);
    }

    #[test]
    fn nonzero_exit_with_no_results_is_an_error() {
        assert!(parse_bicep_lint(&sarif(""), Some(1), &[FILE]).is_err());
        assert!(parse_bicep_lint(&sarif(""), None, &[FILE]).is_err());
    }

    #[test]
    fn malformed_output_is_an_error() {
        assert!(parse_bicep_lint(b"", Some(0), &[FILE]).is_err());
        assert!(parse_bicep_lint(b"Unhandled exception", Some(1), &[FILE]).is_err());
        assert!(parse_bicep_lint(b"{\"runs\":", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn result_in_an_unknown_file_is_an_error() {
        let results = result("BCP033", "error", "/elsewhere/b.bicep", 1, 1);
        assert!(parse_bicep_lint(&sarif(&results), Some(1), &[FILE]).is_err());
    }

    #[test]
    fn result_in_a_known_dependency_is_kept_for_the_caller_to_filter() {
        let dependency = "/scratch/lib.bicep";
        let results = result("BCP033", "error", dependency, 1, 1);
        let found = parse_bicep_lint(&sarif(&results), Some(1), &[FILE, dependency]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, dependency);
    }

    #[test]
    fn format_clean_and_unformatted() {
        let source = b"param x string\n";
        assert!(parse_bicep_format(source, b"", Some(0), source, FILE)
            .unwrap()
            .is_empty());
        let found = parse_bicep_format(source, b"", Some(0), b"param   x string\n", FILE).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.start.line, 1);
        let found = parse_bicep_format(b"a\nb\n", b"", Some(0), b"a\nb  \n", FILE).unwrap();
        assert_eq!(found[0].finding.start.line, 2);
    }

    #[test]
    fn format_failures_are_errors() {
        assert!(parse_bicep_format(b"", b"", Some(0), b"param x string\n", FILE).is_err());
        assert!(parse_bicep_format(b"param x string\n", b"", Some(1), b"", FILE).is_err());
        assert!(parse_bicep_format(b"param x string\n", b"", None, b"", FILE).is_err());
        assert!(parse_bicep_format(b"", b"", Some(0), b"", FILE)
            .unwrap()
            .is_empty());
    }
}
