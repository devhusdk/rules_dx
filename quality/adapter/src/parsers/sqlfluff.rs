use std::collections::BTreeSet;

use serde::Deserialize;

use super::{as_text, code_name, known_spelled, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "sqlfluff";

/// `sqlfluff lint --format json` prints one array with an entry per linted file. Unknown keys
/// (`statistics`, `timings`, `fixes`) are ignored.
#[derive(Debug, Deserialize)]
struct Entry {
    filepath: String,
    violations: Vec<Violation>,
}

#[derive(Debug, Deserialize)]
struct Violation {
    code: String,
    description: String,
    start_line_no: u64,
    start_line_pos: u64,
    #[serde(default)]
    end_line_no: Option<u64>,
    #[serde(default)]
    end_line_pos: Option<u64>,
    #[serde(default)]
    warning: bool,
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .trim()
        .to_owned()
}

/// Parses `sqlfluff lint --format json`. Exit 0 means no violation counted as an error (warnings
/// do not change the exit code), exit 1 means at least one, and exit 2 is a usage or execution
/// error whose reason is on stderr. Unparsable (`PRS`) and templating (`TMP`) violations are
/// error findings. Every passed file must appear in the report: sqlfluff skips a file above
/// `large_file_skip_byte_limit` or matched by an ignore file without reporting it, and a skipped
/// file is never a clean one.
pub fn parse_sqlfluff(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        let reason = if stderr.iter().all(u8::is_ascii_whitespace) {
            first_line(stdout)
        } else {
            first_line(stderr)
        };
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("exit {}: {reason}", code_name(code)),
        });
    }
    let report = as_text(TOOL, stdout)?;
    let entries: Vec<Entry> =
        serde_json::from_str(report.trim()).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    let mut findings = Vec::new();
    let mut errors = 0_usize;
    for entry in &entries {
        let checked = known_spelled(TOOL, files, &entry.filepath)?;
        if !reported.insert(checked) {
            return Err(ParseError::shape(
                TOOL,
                format!("{} appears twice in the report", entry.filepath),
            ));
        }
        for violation in &entry.violations {
            if violation.start_line_no == 0 || violation.start_line_pos == 0 {
                return Err(ParseError::shape(TOOL, "zero start position"));
            }
            if violation.code.is_empty() {
                return Err(ParseError::shape(TOOL, "violation without a code"));
            }
            let severity = if violation.warning {
                ToolSeverity::Warning
            } else {
                errors += 1;
                ToolSeverity::Error
            };
            let end = match (violation.end_line_no, violation.end_line_pos) {
                (Some(line), Some(column)) if line > 0 && column > 0 => {
                    Some(TextPosition { line, column })
                }
                _ => None,
            };
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: violation.code.clone(),
                    message: violation.description.trim().to_owned(),
                    severity,
                    start: TextPosition {
                        line: violation.start_line_no,
                        column: violation.start_line_pos,
                    },
                    end,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
                },
            });
        }
    }
    if (errors > 0) != (code == Some(1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {errors} error violations", code_name(code)),
        ));
    }
    if let Some(missing) = files.iter().find(|file| !reported.contains(**file)) {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!(
                "{missing} is missing from the report; sqlfluff skips files above `large_file_skip_byte_limit` (default 20 KB, set it to 0 in the config) and files matched by a .sqlfluffignore"
            ),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/sql/a.sql", "/s/sql/b.sql"];

    const DIRTY: &str = r#"[{"filepath":"/s/sql/a.sql","violations":[{"start_line_no":1,"start_line_pos":10,"code":"LT01","description":"Expected single whitespace.","name":"layout.spacing","warning":false,"fixes":[],"start_file_pos":9,"end_line_no":1,"end_line_pos":11,"end_file_pos":10}],"statistics":{"source_chars":19},"timings":{"parsing":0.1}},{"filepath":"/s/sql/b.sql","violations":[],"statistics":{},"timings":{}}]"#;

    const CLEAN: &str = r#"[{"filepath":"/s/sql/a.sql","violations":[]},{"filepath":"/s/sql/b.sql","violations":[]}]"#;

    const PARSE: &str = r#"[{"filepath":"/s/sql/a.sql","violations":[]},{"filepath":"/s/sql/b.sql","violations":[{"start_line_no":1,"start_line_pos":1,"code":"PRS","description":"Line 1, Position 1: Found unparsable section: 'selec from ('","name":"","warning":false,"start_file_pos":0,"end_line_no":1,"end_line_pos":13,"end_file_pos":12}]}]"#;

    #[test]
    fn sqlfluff_is_clean_when_every_file_is_reported_without_violations() {
        assert!(parse_sqlfluff(CLEAN.as_bytes(), b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn sqlfluff_locates_each_violation_and_ignores_unknown_keys() {
        let findings = parse_sqlfluff(DIRTY.as_bytes(), b"", Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/sql/a.sql");
        assert_eq!(findings[0].finding.tool_id, "sqlfluff");
        assert_eq!(findings[0].finding.rule_id, "LT01");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition {
                    line: 1,
                    column: 10
                },
                Some(TextPosition {
                    line: 1,
                    column: 11
                })
            )
        );
    }

    #[test]
    fn sqlfluff_reports_an_unparsable_file_as_an_error_finding() {
        let findings = parse_sqlfluff(PARSE.as_bytes(), b"", Some(1), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/sql/b.sql");
        assert_eq!(findings[0].finding.rule_id, "PRS");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn sqlfluff_warnings_keep_exit_zero() {
        let report = CLEAN.replacen(
            r#""violations":[]"#,
            r#""violations":[{"start_line_no":2,"start_line_pos":3,"code":"AL01","description":"Implicit aliasing.","warning":true}]"#,
            1,
        );
        let findings = parse_sqlfluff(report.as_bytes(), b"", Some(0), &FILES).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.end, None);
    }

    #[test]
    fn sqlfluff_fails_closed() {
        // Exit and violations disagree.
        assert!(parse_sqlfluff(CLEAN.as_bytes(), b"", Some(1), &FILES).is_err());
        assert!(parse_sqlfluff(DIRTY.as_bytes(), b"", Some(0), &FILES).is_err());
        // A usage or execution error, a signal.
        let err = parse_sqlfluff(
            b"",
            b"User Error: No dialect was specified.\n",
            Some(2),
            &FILES,
        )
        .expect_err("exit 2");
        assert!(err.to_string().contains("No dialect"), "{err}");
        assert!(parse_sqlfluff(CLEAN.as_bytes(), b"", None, &FILES).is_err());
        // A truncated or garbage report.
        assert!(parse_sqlfluff(&DIRTY.as_bytes()[..60], b"", Some(1), &FILES).is_err());
        assert!(parse_sqlfluff(b"error", b"", Some(1), &FILES).is_err());
        assert!(parse_sqlfluff(b"", b"", Some(0), &FILES).is_err());
        assert!(parse_sqlfluff(&[0xff, 0xfe], b"", Some(1), &FILES).is_err());
        // A file that was not passed, or one reported twice.
        assert!(parse_sqlfluff(DIRTY.as_bytes(), b"", Some(1), &["/s/other.sql"]).is_err());
        let twice = CLEAN.replace("b.sql", "a.sql");
        assert!(parse_sqlfluff(twice.as_bytes(), b"", Some(0), &FILES).is_err());
        // A zero position.
        let zero = DIRTY.replace(r#""start_line_pos":10"#, r#""start_line_pos":0"#);
        assert!(parse_sqlfluff(zero.as_bytes(), b"", Some(1), &FILES).is_err());
    }

    #[test]
    fn sqlfluff_never_treats_a_skipped_file_as_clean() {
        let err = parse_sqlfluff(b"[]", b"", Some(0), &FILES).expect_err("both skipped");
        assert!(
            err.to_string().contains("large_file_skip_byte_limit"),
            "{err}"
        );
        let one = r#"[{"filepath":"/s/sql/a.sql","violations":[]}]"#;
        assert!(parse_sqlfluff(one.as_bytes(), b"", Some(0), &FILES).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn sqlfluff_accepts_either_separator_on_windows() {
        let files = ["C:\\s\\sql\\a.sql"];
        let report = r#"[{"filepath":"C:/s/sql/a.sql","violations":[]}]"#;
        assert!(parse_sqlfluff(report.as_bytes(), b"", Some(0), &files).is_ok());
    }
}
