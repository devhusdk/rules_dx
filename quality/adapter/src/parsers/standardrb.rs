use std::path::Path;

use super::{rubocop_json, FileFinding, ParseError};

/// Parses `standardrb --format json`, which is RuboCop's own JSON grammar.
pub fn parse_standardrb(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    rubocop_json("standardrb", stdout, code, files, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolSeverity;
    const DIRTY: &str = r#"{"files": [{"path": "Sample.rb", "offenses": [{"severity": "convention", "message": "Prefer double-quoted strings.", "cop_name": "Style/StringLiterals", "correctable": true, "status": "uncorrected", "location": {"start_line": 3, "start_column": 1, "last_line": 3, "last_column": 2, "length": 1, "line": 3, "column": 1}}], "summary": {"offense_count": 1}}]}"#;
    const CLEAN: &str =
        r#"{"files": [{"path": "Sample.rb", "offenses": []}], "summary": {"offense_count": 0}}"#;
    #[test]
    fn standardrb_reports_rubocop_json_offenses() {
        let root = Path::new("");
        let findings =
            parse_standardrb(DIRTY.as_bytes(), Some(1), &["Sample.rb"], root).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.tool_id, "standardrb");
        assert_eq!(findings[0].finding.rule_id, "Style/StringLiterals");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let clean =
            parse_standardrb(CLEAN.as_bytes(), Some(0), &["Sample.rb"], root).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_standardrb(b"", Some(1), &["Sample.rb"], root).is_err());
        assert!(parse_standardrb(DIRTY.as_bytes(), Some(1), &["other.rb"], root).is_err());
        assert!(parse_standardrb(&[0xff], Some(0), &["x"], root).is_err());
        let diff = b"--- a/Sample.rb\n+++ b/Sample.rb\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
        assert!(parse_standardrb(diff, Some(1), &["Sample.rb"], root).is_err());
    }
    #[test]
    fn standardrb_names_files_relative_to_the_run_directory() {
        let cwd = Path::new("/scratch");
        let findings = parse_standardrb(DIRTY.as_bytes(), Some(1), &["/scratch/Sample.rb"], cwd)
            .expect("parsed");
        assert_eq!(findings[0].file, "/scratch/Sample.rb");
    }
}
