use std::path::Path;

use super::{rubocop_json, FileFinding, ParseError};

/// Parses `rubocop --format json`, naming files as given or relative to `cwd`.
pub fn parse_rubocop(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    rubocop_json("rubocop", stdout, stderr, code, files, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolSeverity;
    const DIRTY: &str = r#"{"files": [{"path": "Sample.rb", "offenses": [{"severity": "convention", "message": "Use double quotes", "cop_name": "Style/StringLiterals", "location": {"line": 3, "column": 1}}]}]}"#;
    const CLEAN: &str = r#"{"files": [{"path": "Sample.rb", "offenses": []}]}"#;
    const SYNTAX: &str = r#"{"files": [{"path": "bad.rb", "offenses": [{"severity": "fatal", "message": "unexpected token $end", "cop_name": "Lint/Syntax", "location": {"line": 2, "column": 1}}]}]}"#;
    const INFO: &str = r#"{"files": [{"path": "Sample.rb", "offenses": [{"severity": "info", "message": "Note", "cop_name": "Style/Note", "location": {"line": 1, "column": 2}}]}]}"#;
    #[test]
    fn rubocop_reports_json_offenses() {
        let root = Path::new("");
        let findings =
            parse_rubocop(DIRTY.as_bytes(), b"", Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "Style/StringLiterals");
        let clean =
            parse_rubocop(CLEAN.as_bytes(), b"", Some(0), &["Sample.rb"], root).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_rubocop(b"", b"", Some(1), &["Sample.rb"], root).is_err());
        assert!(parse_rubocop(DIRTY.as_bytes(), b"", Some(1), &["other.rb"], root).is_err());
        assert!(parse_rubocop(&[0xff], b"", Some(0), &["x"], root).is_err());
    }
    #[test]
    fn rubocop_names_files_relative_to_the_run_directory() {
        let cwd = Path::new("/scratch");
        let findings = parse_rubocop(DIRTY.as_bytes(), b"", Some(1), &["/scratch/Sample.rb"], cwd)
            .expect("parsed");
        assert_eq!(findings[0].file, "/scratch/Sample.rb");
        assert!(parse_rubocop(DIRTY.as_bytes(), b"", Some(1), &["/other/Sample.rb"], cwd).is_err());
    }
    #[test]
    fn rubocop_syntax_errors_are_errors_and_info_is_info() {
        let root = Path::new("");
        let syntax =
            parse_rubocop(SYNTAX.as_bytes(), b"", Some(1), &["bad.rb"], root).expect("parsed");
        assert_eq!(syntax[0].finding.severity, ToolSeverity::Error);
        assert_eq!(syntax[0].finding.rule_id, "Lint/Syntax");
        let info =
            parse_rubocop(INFO.as_bytes(), b"", Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(info[0].finding.severity, ToolSeverity::Info);
    }
    #[test]
    fn rubocop_failing_without_offenses_is_an_error() {
        let root = Path::new("");
        assert!(parse_rubocop(CLEAN.as_bytes(), b"", Some(2), &["Sample.rb"], root).is_err());
        let err = parse_rubocop(CLEAN.as_bytes(), b"", Some(1), &["Sample.rb"], root)
            .expect_err("exit 1 without offenses");
        assert!(
            err.to_string().contains("exit 1 with no diagnostics"),
            "{err}"
        );
    }
    #[test]
    fn rubocop_cop_crash_beside_an_offense_is_an_error() {
        let root = Path::new("");
        let stderr =
            b"\n1 error occurred:\nAn error occurred while Style/Foo cop was inspecting a.rb.\n";
        let err = parse_rubocop(DIRTY.as_bytes(), stderr, Some(1), &["Sample.rb"], root)
            .expect_err("crash");
        let text = err.to_string();
        assert!(
            text.contains(
                "cop crashed: An error occurred while Style/Foo cop was inspecting a.rb."
            ),
            "{text}"
        );
        let err = parse_rubocop(CLEAN.as_bytes(), stderr, Some(1), &["Sample.rb"], root)
            .expect_err("crash without offenses");
        assert!(err.to_string().contains("cop crashed"), "{err}");
        assert!(
            parse_rubocop(b"not json", stderr, Some(0), &["Sample.rb"], root)
                .expect_err("crash before json")
                .to_string()
                .contains("cop crashed")
        );
        let summary = b"\x1b[31m2 errors occurred:\x1b[0m\n";
        let err = parse_rubocop(DIRTY.as_bytes(), summary, Some(1), &["Sample.rb"], root)
            .expect_err("summary only");
        assert!(
            err.to_string().contains("cop crashed: 2 errors occurred:"),
            "{err}"
        );
        let bytes = b"1 error occurred:\n\xff\xfe backtrace\n";
        assert!(parse_rubocop(DIRTY.as_bytes(), bytes, Some(1), &["Sample.rb"], root).is_err());
    }
    #[test]
    fn rubocop_config_error_with_offenses_is_an_error() {
        let root = Path::new("");
        let stderr = b"Error: unrecognized cop or department Foo/Bar\n";
        let err = parse_rubocop(DIRTY.as_bytes(), stderr, Some(2), &["Sample.rb"], root)
            .expect_err("exit 2");
        let text = err.to_string();
        assert!(text.contains("exit 2"), "{text}");
        assert!(
            text.contains("Error: unrecognized cop or department Foo/Bar"),
            "{text}"
        );
        let err = parse_rubocop(b"", b"", None, &["Sample.rb"], root).expect_err("signal");
        assert!(err.to_string().contains("exit signal : no stderr"), "{err}");
    }
    #[test]
    fn rubocop_warnings_on_stderr_do_not_fail() {
        let root = Path::new("");
        let stderr = b"Warning: Lint/Foo has the wrong namespace - should be Style\n\
            Style/Bar has been deprecated. 1 errors are not counted.\n";
        let findings =
            parse_rubocop(DIRTY.as_bytes(), stderr, Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(findings.len(), 1);
        let info = parse_rubocop(INFO.as_bytes(), b"", Some(0), &["Sample.rb"], root)
            .expect("info only exits 0");
        assert_eq!(info[0].finding.severity, ToolSeverity::Info);
    }
}
