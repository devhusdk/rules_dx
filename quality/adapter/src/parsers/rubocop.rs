use std::path::Path;

use super::{rubocop_json, FileFinding, ParseError};

/// Parses `rubocop --format json`, naming files as given or relative to `cwd`.
pub fn parse_rubocop(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    rubocop_json("rubocop", stdout, code, files, cwd)
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
            parse_rubocop(DIRTY.as_bytes(), Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "Style/StringLiterals");
        let clean = parse_rubocop(CLEAN.as_bytes(), Some(0), &["Sample.rb"], root).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_rubocop(b"", Some(1), &["Sample.rb"], root).is_err());
        assert!(parse_rubocop(DIRTY.as_bytes(), Some(1), &["other.rb"], root).is_err());
        assert!(parse_rubocop(&[0xff], Some(0), &["x"], root).is_err());
    }
    #[test]
    fn rubocop_names_files_relative_to_the_run_directory() {
        let cwd = Path::new("/scratch");
        let findings =
            parse_rubocop(DIRTY.as_bytes(), Some(1), &["/scratch/Sample.rb"], cwd).expect("parsed");
        assert_eq!(findings[0].file, "/scratch/Sample.rb");
        assert!(parse_rubocop(DIRTY.as_bytes(), Some(1), &["/other/Sample.rb"], cwd).is_err());
    }
    #[test]
    fn rubocop_syntax_errors_are_errors_and_info_is_info() {
        let root = Path::new("");
        let syntax = parse_rubocop(SYNTAX.as_bytes(), Some(1), &["bad.rb"], root).expect("parsed");
        assert_eq!(syntax[0].finding.severity, ToolSeverity::Error);
        assert_eq!(syntax[0].finding.rule_id, "Lint/Syntax");
        let info = parse_rubocop(INFO.as_bytes(), Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(info[0].finding.severity, ToolSeverity::Info);
    }
    #[test]
    fn rubocop_failing_without_offenses_is_an_error() {
        let root = Path::new("");
        assert!(parse_rubocop(CLEAN.as_bytes(), Some(2), &["Sample.rb"], root).is_err());
    }
}
