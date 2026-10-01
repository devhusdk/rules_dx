use super::{rubocop_json, FileFinding, ParseError};

/// Report the offenses `standardrb --format json` writes, which are RuboCop's own JSON grammar.
///
/// `standardrb` wraps RuboCop and forwards `--format` to it, so both tools name every offense
/// with the same `cop_name`, `severity` and `location` fields.
pub fn parse_standardrb(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    rubocop_json("standardrb", stdout, code, files)
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
        let findings = parse_standardrb(DIRTY.as_bytes(), Some(1), &["Sample.rb"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.tool_id, "standardrb");
        assert_eq!(findings[0].finding.rule_id, "Style/StringLiterals");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let clean = parse_standardrb(CLEAN.as_bytes(), Some(0), &["Sample.rb"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_standardrb(b"", Some(1), &["Sample.rb"]).is_err());
        assert!(parse_standardrb(DIRTY.as_bytes(), Some(1), &["other.rb"]).is_err());
        assert!(parse_standardrb(&[0xff], Some(0), &["x"]).is_err());
        let diff = b"--- a/Sample.rb\n+++ b/Sample.rb\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
        assert!(parse_standardrb(diff, Some(1), &["Sample.rb"]).is_err());
    }
}
